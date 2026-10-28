#include <libmtp.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc, char **argv) {
  uint32_t parent = (uint32_t)strtoul(argv[1], NULL, 10);
  LIBMTP_Init();
  LIBMTP_raw_device_t *raw; int nraw;
  if (LIBMTP_Detect_Raw_Devices(&raw, &nraw) != 0 || nraw < 1) { fprintf(stderr, "no device\n"); return 1; }
  LIBMTP_mtpdevice_t *dev = LIBMTP_Open_Raw_Device_Uncached(&raw[0]);
  if (!dev) { fprintf(stderr, "open failed\n"); return 1; }
  printf("get_storage=%d\n", LIBMTP_Get_Storage(dev, LIBMTP_STORAGE_SORTBY_NOTSORTED));
  for (LIBMTP_devicestorage_t *st = dev->storage; st; st = st->next) {
    printf("storage %08x %s\n", st->id, st->StorageDescription);
    for (LIBMTP_file_t *f = LIBMTP_Get_Files_And_Folders(dev, st->id, parent); f; f = f->next)
      printf("  %u %s %llu\n", f->item_id, f->filename, (unsigned long long)f->filesize);
  }
  LIBMTP_Dump_Errorstack(dev);
  LIBMTP_Release_Device(dev);
}
