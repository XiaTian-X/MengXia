# Closed parser for LC_ALL=C /bin/ls -ldben output (one path only).
# Do not resolve groups or silently remove ACLs: mutating allow ACEs are unsupported.
function reject() { bad=1; exit 1 }
{ bytes += length($0)+1; if (bytes>16384) reject() }
NR==1 {
    split($0, header, / +/); mode=header[1]
    if (length(mode)<10 || length(mode)>11 || substr(mode,1,1)!~/^[-dl]$/) reject()
    bits=substr(mode,2,9); if (bits~/[^rwxstST-]/) reject()
    marker=substr(mode,11)
    if (marker!="" && marker!="+" && marker!="@") reject()
    next
}
{
    line=$0; sub(/^ +/, "", line); n=split(line, a, / +/)
    if (++entries>128 || a[1]!=(entries-1) ":") reject()
    if (length(a[2])!=36 || a[2]~/[^0-9A-Fa-f-]/) reject()
    field=3; if (a[field]=="inherited") field++
    kind=a[field++]
    if ((kind!="allow" && kind!="deny") || n!=field) reject()
    count=split(a[field], rights, ","); if (!count) reject()
    for (i=1; i<=count; i++) {
        right=rights[i]
        if (right!~/^(read|list|execute|search|readattr|readextattr|readsecurity|write|add_file|append|add_subdirectory|delete|delete_child|writeattr|writeextattr|writesecurity|chown|file_inherit|directory_inherit|limit_inherit|only_inherit)$/) reject()
        if (kind=="allow" && right~/^(write|add_file|append|add_subdirectory|delete|delete_child|writeattr|writeextattr|writesecurity|chown)$/) reject()
    }
}
END { if (bad || NR==0 || (marker=="+" && entries==0)) exit 1 }
