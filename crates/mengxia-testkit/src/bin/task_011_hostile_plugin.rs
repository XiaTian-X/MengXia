//! Test-only hostile Plugin process. This target intentionally uses only `std`.

#![forbid(unsafe_code)]

use std::env;
use std::io::{self, Read, Write};
use std::thread;
use std::time::Duration;

const FRAME_LIMIT: u32 = 65_536;

fn main() {
    let action = env::args().nth(1).unwrap_or_default();
    match action.as_str() {
        "zero_frame" => raw(&0_u32.to_be_bytes()),
        "oversized_frame" | "stdout_flood" => raw(&(FRAME_LIMIT + 1).to_be_bytes()),
        "truncated_header" => raw(&[0, 1]),
        "truncated_payload" => raw(&[0, 0, 0, 8, 0xaa]),
        "unknown_field" | "core_protocol_frame" => frame(&[0x30, 0x01]),
        "duplicate_field" => frame(&[0x08, 0x01, 0x08, 0x02]),
        "nonminimal_varint" => frame(&[0x08, 0x80, 0x00]),
        "wrong_wire_type" => frame(&[0x0a, 0x00]),
        "stdout_text" => raw(b"text"),
        "close_before_hello" => {}
        "hang_before_hello" => hang(),
        _ => after_hello(&action),
    }
}

fn after_hello(action: &str) {
    let Some(host_hello) = read_frame() else {
        return;
    };
    let challenge = embedded_field(&host_hello, 2)
        .and_then(|hello| bytes_field(hello, 3))
        .unwrap_or(&[]);
    let reply_challenge = if action == "wrong_challenge" {
        vec![0x7f; 32]
    } else {
        challenge.to_vec()
    };
    let version = if action == "wrong_version" { 2 } else { 1 };
    if action == "failure_during_hello" {
        frame(&plugin_failure(0, 1));
        return;
    }
    frame(&plugin_hello(version, &reply_challenge));
    match action {
        "close_during_response" => {
            let _ = read_frame();
        }
        "hang_during_response" | "ignore_shutdown" => {
            let _ = read_frame();
            hang();
        }
        "panic_after_hello" => panic!("test-only hostile fixture panic"),
        "stderr_at_cap" => {
            write_stderr(65_536);
            serve_valid();
        }
        "stderr_cap_plus_one" | "stderr_flood" => {
            write_stderr(65_537);
            hang();
        }
        "wrong_sequence" => respond_to_one(action, 2),
        "ping_failure_unsupported" => respond_failure(1),
        "ping_failure_invalid" => respond_failure(2),
        "ping_failure_internal" => respond_failure(3),
        "shutdown_failure_unsupported" => respond_shutdown_failure(1),
        "shutdown_failure_invalid" => respond_shutdown_failure(2),
        "shutdown_failure_internal" => respond_shutdown_failure(3),
        "failure_unspecified" => respond_failure(0),
        "failure_unknown" => respond_failure(4),
        "duplicate_failure" => {
            let Some(request) = read_frame() else { return };
            let sequence = varint_field(&request, 1).unwrap_or(1);
            frame(&plugin_failure(sequence, 1));
            frame(&plugin_failure(sequence, 1));
            hang();
        }
        "valid_ping" => serve_valid(),
        _ => respond_to_one(action, 1),
    }
}

fn respond_to_one(action: &str, sequence_override: u64) {
    let Some(request) = read_frame() else { return };
    let sequence = varint_field(&request, 1).unwrap_or(1);
    let sequence = if action == "wrong_sequence" {
        sequence_override
    } else {
        sequence
    };
    if let Some(ping) = embedded_field(&request, 3) {
        let Some(nonce) = fixed64_field(ping, 1) else {
            return;
        };
        frame(&ping_response(sequence, nonce));
    } else {
        frame(&shutdown_response(sequence));
    }
}

fn serve_valid() {
    while let Some(request) = read_frame() {
        let sequence = varint_field(&request, 1).unwrap_or(0);
        if let Some(ping) = embedded_field(&request, 3) {
            let Some(nonce) = fixed64_field(ping, 1) else {
                return;
            };
            frame(&ping_response(sequence, nonce));
        } else if embedded_field(&request, 4).is_some() {
            frame(&shutdown_response(sequence));
            return;
        } else {
            return;
        }
    }
}

fn respond_failure(code: u64) {
    let Some(request) = read_frame() else { return };
    frame(&plugin_failure(
        varint_field(&request, 1).unwrap_or(1),
        code,
    ));
    if (1..=3).contains(&code) {
        serve_valid();
    }
}

fn respond_shutdown_failure(code: u64) {
    let Some(request) = read_frame() else { return };
    if embedded_field(&request, 3).is_some() {
        let sequence = varint_field(&request, 1).unwrap_or(1);
        let Some(nonce) = embedded_field(&request, 3).and_then(|ping| fixed64_field(ping, 1))
        else {
            return;
        };
        frame(&ping_response(sequence, nonce));
    }
    let Some(shutdown) = read_frame() else { return };
    frame(&plugin_failure(
        varint_field(&shutdown, 1).unwrap_or(2),
        code,
    ));
}

fn plugin_hello(major: u64, challenge: &[u8]) -> Vec<u8> {
    let mut hello = vec![0x08];
    put_varint(&mut hello, major);
    hello.extend_from_slice(&[0x1a]);
    put_varint(&mut hello, challenge.len() as u64);
    hello.extend_from_slice(challenge);
    envelope(0, 2, &hello)
}

fn ping_response(sequence: u64, nonce: u64) -> Vec<u8> {
    let mut ping = vec![0x09];
    ping.extend_from_slice(&nonce.to_le_bytes());
    envelope(sequence, 3, &ping)
}

fn shutdown_response(sequence: u64) -> Vec<u8> {
    envelope(sequence, 4, &[])
}

fn plugin_failure(sequence: u64, code: u64) -> Vec<u8> {
    let mut failure = vec![0x08];
    put_varint(&mut failure, code);
    envelope(sequence, 5, &failure)
}

fn envelope(sequence: u64, body_field: u8, body: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    if sequence != 0 {
        output.push(0x08);
        put_varint(&mut output, sequence);
    }
    output.push(body_field << 3 | 2);
    put_varint(&mut output, body.len() as u64);
    output.extend_from_slice(body);
    output
}

fn frame(payload: &[u8]) {
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&(payload.len() as u32).to_be_bytes())
        .unwrap();
    stdout.write_all(payload).unwrap();
    stdout.flush().unwrap();
}

fn raw(bytes: &[u8]) {
    let mut stdout = io::stdout().lock();
    stdout.write_all(bytes).unwrap();
    stdout.flush().unwrap();
}

fn read_frame() -> Option<Vec<u8>> {
    let mut header = [0_u8; 4];
    io::stdin().read_exact(&mut header).ok()?;
    let length = u32::from_be_bytes(header) as usize;
    let mut payload = vec![0_u8; length];
    io::stdin().read_exact(&mut payload).ok()?;
    Some(payload)
}

fn write_stderr(length: usize) {
    let chunk = [b'x'; 1024];
    let mut stderr = io::stderr().lock();
    let mut remaining = length;
    while remaining > 0 {
        let count = remaining.min(chunk.len());
        stderr.write_all(&chunk[..count]).unwrap();
        remaining -= count;
    }
    stderr.flush().unwrap();
}

fn hang() -> ! {
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn put_varint(output: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            output.push(byte);
            return;
        }
        output.push(byte | 0x80);
    }
}

fn varint_field(input: &[u8], wanted: u64) -> Option<u64> {
    let mut offset = 0;
    while offset < input.len() {
        let key = get_varint(input, &mut offset)?;
        let field = key >> 3;
        let wire = key & 7;
        if wire == 0 {
            let value = get_varint(input, &mut offset)?;
            if field == wanted {
                return Some(value);
            }
        } else {
            skip(input, &mut offset, wire)?;
        }
    }
    None
}

fn fixed64_field(input: &[u8], wanted: u64) -> Option<u64> {
    let mut offset = 0;
    while offset < input.len() {
        let key = get_varint(input, &mut offset)?;
        let field = key >> 3;
        let wire = key & 7;
        if wire == 1 {
            let end = offset.checked_add(8)?;
            let bytes: [u8; 8] = input.get(offset..end)?.try_into().ok()?;
            offset = end;
            if field == wanted {
                return Some(u64::from_le_bytes(bytes));
            }
        } else {
            skip(input, &mut offset, wire)?;
        }
    }
    None
}

fn embedded_field(input: &[u8], wanted: u64) -> Option<&[u8]> {
    bytes_field(input, wanted)
}

fn bytes_field(input: &[u8], wanted: u64) -> Option<&[u8]> {
    let mut offset = 0;
    while offset < input.len() {
        let key = get_varint(input, &mut offset)?;
        let field = key >> 3;
        let wire = key & 7;
        if wire == 2 {
            let length = get_varint(input, &mut offset)? as usize;
            let end = offset.checked_add(length)?;
            let bytes = input.get(offset..end)?;
            offset = end;
            if field == wanted {
                return Some(bytes);
            }
        } else {
            skip(input, &mut offset, wire)?;
        }
    }
    None
}

fn get_varint(input: &[u8], offset: &mut usize) -> Option<u64> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *input.get(*offset)?;
        *offset += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn skip(input: &[u8], offset: &mut usize, wire: u64) -> Option<()> {
    let width = match wire {
        0 => {
            get_varint(input, offset)?;
            return Some(());
        }
        1 => 8,
        2 => get_varint(input, offset)? as usize,
        5 => 4,
        _ => return None,
    };
    *offset = offset.checked_add(width)?;
    input.get(..*offset)?;
    Some(())
}
