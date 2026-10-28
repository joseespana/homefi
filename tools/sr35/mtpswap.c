// mtpswap <folder id> <remote name> <backup path> <new local file>
// In one MTP session: back up <remote name>, verify its size, delete it, upload the new file under that name.
#include <libmtp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

int main(int argc, char **argv) {
  if (argc != 5) { fprintf(stderr, "usage: mtpswap <folder id> <name> <backup> <new>\n"); return 2; }
  uint32_t parent = (uint32_t)strtoul(argv[1], NULL, 10);
  LIBMTP_Init();
  // the SR35 rejects per-folder queries; the full listing (as mtp-files does) works
  LIBMTP_mtpdevice_t *dev = LIBMTP_Get_First_Device();
  if (!dev) { fprintf(stderr, "no device\n"); return 1; }
  LIBMTP_file_t *f;
  for (f = LIBMTP_Get_Filelisting_With_Callback(dev, NULL, NULL); f; f = f->next)
    if (f->parent_id == parent && strcmp(f->filename, argv[2]) == 0) break;
  if (!f) { fprintf(stderr, "%s not found in folder %u\n", argv[2], parent); return 1; }
  uint32_t storage = f->storage_id;
  printf("found %s id %u size %llu\n", f->filename, f->item_id, (unsigned long long)f->filesize);
  if (LIBMTP_Get_File_To_File(dev, f->item_id, argv[3], NULL, NULL) != 0) { fprintf(stderr, "backup failed\n"); return 1; }
  struct stat sb;
  if (stat(argv[3], &sb) != 0 || (uint64_t)sb.st_size != f->filesize) { fprintf(stderr, "backup size mismatch, aborting\n"); return 1; }
  printf("backup ok (%lld bytes)\n", (long long)sb.st_size);
  if (LIBMTP_Delete_Object(dev, f->item_id) != 0) { fprintf(stderr, "delete failed\n"); return 1; }
  if (stat(argv[4], &sb) != 0) { perror(argv[4]); return 1; }
  LIBMTP_file_t *n = LIBMTP_new_file_t();
  n->filename = strdup(argv[2]); n->filesize = sb.st_size; n->filetype = LIBMTP_FILETYPE_UNKNOWN;
  n->parent_id = parent; n->storage_id = storage;
  if (LIBMTP_Send_File_From_File(dev, argv[4], n, NULL, NULL) != 0) { fprintf(stderr, "upload failed\n"); LIBMTP_Dump_Errorstack(dev); return 1; }
  printf("uploaded %s as %s (%lld bytes)\n", argv[4], argv[2], (long long)sb.st_size);
  LIBMTP_Release_Device(dev);
  return 0;
}
