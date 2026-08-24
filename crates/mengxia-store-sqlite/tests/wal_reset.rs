#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

const CHILD_ENV: &str = "MENGXIA_TASK004_WAL_RESET_CHILD";
const FIXTURE_ENV: &str = "MENGXIA_TASK004_WAL_RESET_PATH";
const SEEDS: u32 = 16;
const CYCLES: u32 = 256;
const WATCHDOG: Duration = Duration::from_secs(30);

#[test]
fn deterministic_multi_connection_wal_reset_regression() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run_child_matrix();
        return;
    }

    let fixture = fixture_root();
    let mut child = Command::new(std::env::current_exe().expect("current WAL test executable"))
        .args([
            "--exact",
            "deterministic_multi_connection_wal_reset_regression",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .env(FIXTURE_ENV, &fixture)
        .spawn()
        .expect("spawn WAL-reset watchdog child");
    let deadline = Instant::now() + WATCHDOG;
    loop {
        if let Some(status) = child.try_wait().expect("poll WAL-reset child") {
            assert!(status.success(), "WAL-reset child failed: {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill hung WAL-reset child");
            let _ = child.wait();
            panic!("WAL-reset matrix exceeded the independent 30 second watchdog");
        }
        thread::sleep(Duration::from_millis(10));
    }
    fs::remove_dir_all(fixture).expect("remove WAL-reset fixture root");
}

fn run_child_matrix() {
    let fixture =
        PathBuf::from(std::env::var_os(FIXTURE_ENV).expect("WAL-reset child fixture path"));
    fs::create_dir(&fixture).expect("create standalone WAL-reset fixture root");
    for seed in 0..SEEDS {
        run_seed(&fixture, seed);
    }
}

fn run_seed(fixture: &Path, seed: u32) {
    let path = fixture.join(format!("seed-{seed}.sqlite3"));
    let setup = hardened_connection(&path);
    setup
        .execute_batch(
            "CREATE TABLE wal_reset_probe (
                sequence INTEGER PRIMARY KEY NOT NULL,
                writer INTEGER NOT NULL,
                payload BLOB NOT NULL,
                checksum BLOB NOT NULL CHECK (length(checksum) = 32)
            ) STRICT;",
        )
        .expect("create isolated test-only WAL schema");
    insert_probe(&setup, seed, 0, 0).expect("commit baseline probe");

    let reader = hardened_connection(&path);
    reader
        .execute_batch("BEGIN")
        .expect("begin retained snapshot");
    let baseline: i64 = reader
        .query_row("SELECT count(*) FROM wal_reset_probe", [], |row| row.get(0))
        .expect("materialize retained reader snapshot");
    assert_eq!(baseline, 1);

    let writer_a = WriterWorker::start(hardened_connection(&path));
    let writer_b = WriterWorker::start(hardened_connection(&path));
    let checkpointer = CheckpointWorker::start(hardened_connection(&path));

    writer_a.insert(seed, 1, 1);
    writer_b.insert(seed, 2, 2);
    let retained = checkpointer.checkpoint("FULL");
    assert!(
        retained.busy != 0 || retained.checkpointed < retained.log,
        "seed {seed}: retained reader did not constrain FULL checkpoint: {retained:?}"
    );
    reader
        .execute_batch("ROLLBACK")
        .expect("release retained snapshot");
    drop(reader);

    let quiescent = checkpointer.checkpoint("FULL");
    assert_eq!(quiescent.busy, 0, "seed {seed}: quiescent FULL busy");
    assert_eq!(
        quiescent.checkpointed, quiescent.log,
        "seed {seed}: quiescent FULL incomplete"
    );
    let restart = checkpointer.checkpoint("RESTART");
    assert_eq!(restart.busy, 0, "seed {seed}: initial RESTART busy");

    let wal_path = PathBuf::from(format!("{}-wal", path.display()));
    let mut observed_salts = BTreeSet::new();
    for cycle in 0..CYCLES {
        let sequence = i64::from(cycle) + 3;
        let (writer, writer_id) = if (cycle.wrapping_add(seed) & 1) == 0 {
            (&writer_a, 1_i64)
        } else {
            (&writer_b, 2_i64)
        };

        writer.begin_insert(seed, sequence, writer_id);
        checkpointer.begin_checkpoint(if cycle & 1 == 0 { "RESTART" } else { "FULL" });
        writer.finish_insert();
        let raced = checkpointer.finish_checkpoint();
        assert!(
            raced.busy == 0 || raced.busy == 1,
            "seed {seed} cycle {cycle}: invalid checkpoint busy result {raced:?}"
        );
        observed_salts.insert(read_wal_salt(&wal_path));

        let required_restart = checkpointer.checkpoint("RESTART");
        assert_eq!(
            required_restart.busy, 0,
            "seed {seed} cycle {cycle}: quiescent RESTART busy"
        );
    }
    assert!(
        observed_salts.len() > 1,
        "seed {seed}: coordinated RESTART never produced a new WAL salt"
    );

    let truncated = checkpointer.checkpoint("TRUNCATE");
    assert_eq!(truncated.busy, 0, "seed {seed}: final TRUNCATE busy");
    assert_eq!(truncated.log, 0, "seed {seed}: final WAL was not empty");
    assert_eq!(
        truncated.checkpointed, 0,
        "seed {seed}: final TRUNCATE retained frames"
    );
    writer_a.stop();
    writer_b.stop();
    checkpointer.stop();
    drop(setup);

    assert!(
        !wal_path.exists() || fs::metadata(&wal_path).expect("final WAL metadata").len() == 0,
        "seed {seed}: final WAL has bytes"
    );
    let reopened = hardened_connection(&path);
    let quick_check: String = reopened
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .expect("final quick_check");
    assert_eq!(quick_check, "ok", "seed {seed}: final integrity failure");
    let mut rows = reopened
        .prepare(
            "SELECT sequence, writer, payload, checksum
             FROM wal_reset_probe ORDER BY sequence",
        )
        .expect("prepare final acknowledged-commit query");
    let actual = rows
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, Vec<u8>>(3)?,
            ))
        })
        .expect("query final acknowledged commits")
        .collect::<Result<Vec<_>, _>>()
        .expect("read final acknowledged commits");
    assert_eq!(
        actual.len(),
        CYCLES as usize + 3,
        "seed {seed}: missing or duplicate commit"
    );
    for (expected_sequence, (sequence, writer, payload, checksum)) in (0_i64..).zip(actual) {
        assert_eq!(sequence, expected_sequence, "seed {seed}: sequence gap");
        let expected_writer = if sequence == 0 {
            0
        } else if sequence == 1 {
            1
        } else if sequence == 2 {
            2
        } else {
            let cycle = (sequence - 3) as u32;
            if cycle.wrapping_add(seed) & 1 == 0 {
                1
            } else {
                2
            }
        };
        assert_eq!(writer, expected_writer, "seed {seed}: writer mismatch");
        assert_eq!(payload, probe_payload(seed, sequence, writer));
        assert_eq!(checksum, Sha256::digest(&payload).as_slice());
    }
    drop(rows);
    drop(reopened);
}

fn hardened_connection(path: &Path) -> Connection {
    assert_eq!(
        rusqlite::version_number(),
        3_053_004,
        "WAL regression must link the TASK-004 SQLite pin"
    );
    let connection = Connection::open(path).expect("open standalone WAL fixture");
    connection
        .busy_timeout(Duration::from_millis(37))
        .expect("set finite busy timeout");
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA trusted_schema = OFF;
             PRAGMA recursive_triggers = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = FULL;
             PRAGMA wal_autocheckpoint = 0;",
        )
        .expect("apply production-equivalent connection hardening");
    for (pragma, expected) in [
        ("foreign_keys", 1_i64),
        ("trusted_schema", 0),
        ("recursive_triggers", 1),
        ("synchronous", 2),
        ("wal_autocheckpoint", 0),
    ] {
        let actual: i64 = connection
            .query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
            .expect("read hardened pragma");
        assert_eq!(actual, expected, "hardened PRAGMA {pragma}");
    }
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("read hardened journal mode");
    assert_eq!(journal_mode, "wal");
    let busy_timeout: i64 = connection
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .expect("read finite busy timeout");
    assert_eq!(busy_timeout, 37);
    connection
}

fn read_wal_salt(path: &Path) -> [u8; 8] {
    let header = fs::read(path).expect("read WAL header at quiescent barrier");
    assert!(header.len() >= 32, "WAL header must be complete");
    header[16..24].try_into().expect("fixed WAL salt width")
}

fn insert_probe(
    connection: &Connection,
    seed: u32,
    sequence: i64,
    writer: i64,
) -> rusqlite::Result<()> {
    let payload = probe_payload(seed, sequence, writer);
    let checksum: [u8; 32] = Sha256::digest(&payload).into();
    connection.execute(
        "INSERT INTO wal_reset_probe (sequence, writer, payload, checksum) VALUES (?1, ?2, ?3, ?4)",
        params![sequence, writer, payload, checksum.as_slice()],
    )?;
    Ok(())
}

fn probe_payload(seed: u32, sequence: i64, writer: i64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(20);
    payload.extend_from_slice(b"MX-WAL\0");
    payload.extend_from_slice(&seed.to_be_bytes());
    payload.extend_from_slice(&sequence.to_be_bytes());
    payload.push(writer as u8);
    payload
}

#[derive(Clone, Copy, Debug)]
struct CheckpointResult {
    busy: i64,
    log: i64,
    checkpointed: i64,
}

enum WriterCommand {
    Insert {
        seed: u32,
        sequence: i64,
        writer: i64,
    },
    Stop,
}

struct WriterWorker {
    commands: SyncSender<WriterCommand>,
    results: Receiver<rusqlite::Result<()>>,
    join: thread::JoinHandle<()>,
}

impl WriterWorker {
    fn start(connection: Connection) -> Self {
        let (commands, command_rx) = mpsc::sync_channel(0);
        let (result_tx, results) = mpsc::sync_channel(0);
        let join = thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                match command {
                    WriterCommand::Insert {
                        seed,
                        sequence,
                        writer,
                    } => result_tx
                        .send(insert_probe(&connection, seed, sequence, writer))
                        .expect("return writer result"),
                    WriterCommand::Stop => break,
                }
            }
        });
        Self {
            commands,
            results,
            join,
        }
    }

    fn begin_insert(&self, seed: u32, sequence: i64, writer: i64) {
        self.commands
            .send(WriterCommand::Insert {
                seed,
                sequence,
                writer,
            })
            .expect("schedule writer commit");
    }

    fn finish_insert(&self) {
        self.results
            .recv()
            .expect("receive writer result")
            .expect("writer commit");
    }

    fn insert(&self, seed: u32, sequence: i64, writer: i64) {
        self.begin_insert(seed, sequence, writer);
        self.finish_insert();
    }

    fn stop(self) {
        self.commands
            .send(WriterCommand::Stop)
            .expect("stop writer worker");
        self.join.join().expect("join writer worker");
    }
}

enum CheckpointCommand {
    Run(&'static str),
    Stop,
}

struct CheckpointWorker {
    commands: SyncSender<CheckpointCommand>,
    results: Receiver<rusqlite::Result<CheckpointResult>>,
    join: thread::JoinHandle<()>,
}

impl CheckpointWorker {
    fn start(connection: Connection) -> Self {
        let (commands, command_rx) = mpsc::sync_channel(0);
        let (result_tx, results) = mpsc::sync_channel(0);
        let join = thread::spawn(move || {
            while let Ok(command) = command_rx.recv() {
                match command {
                    CheckpointCommand::Run(mode) => result_tx
                        .send(run_checkpoint(&connection, mode))
                        .expect("return checkpoint result"),
                    CheckpointCommand::Stop => break,
                }
            }
        });
        Self {
            commands,
            results,
            join,
        }
    }

    fn begin_checkpoint(&self, mode: &'static str) {
        self.commands
            .send(CheckpointCommand::Run(mode))
            .expect("schedule checkpoint");
    }

    fn finish_checkpoint(&self) -> CheckpointResult {
        self.results
            .recv()
            .expect("receive checkpoint result")
            .expect("checkpoint call")
    }

    fn checkpoint(&self, mode: &'static str) -> CheckpointResult {
        self.begin_checkpoint(mode);
        self.finish_checkpoint()
    }

    fn stop(self) {
        self.commands
            .send(CheckpointCommand::Stop)
            .expect("stop checkpoint worker");
        self.join.join().expect("join checkpoint worker");
    }
}

fn run_checkpoint(connection: &Connection, mode: &str) -> rusqlite::Result<CheckpointResult> {
    connection.query_row(&format!("PRAGMA wal_checkpoint({mode})"), [], |row| {
        Ok(CheckpointResult {
            busy: row.get(0)?,
            log: row.get(1)?,
            checkpointed: row.get(2)?,
        })
    })
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("task-004-wal-reset-{}", std::process::id()))
}
