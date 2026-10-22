/* vita-elf-create appends SCE metadata to the executable segment. Leave a
 * full 64 KiB gap before writable data, even when code ends near a boundary. */
SECTIONS
{
  . = ALIGN(0x10000) + 0x10000;
}
INSERT BEFORE .init_array;
