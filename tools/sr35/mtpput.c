// mtpput <local file> <folder name in internal storage> <remote name>
#include <libmtp.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <sys/stat.h>

static uint32_t find(LIBMTP_folder_t *f, const char *name) {
  for (; f; f = f->sibling) {
    if (f->parent_id == 0 && strcmp(f->name, name) == 0) return f->folder_id;
  }
  return 0;
}

int main(int argc, char **argv) {
  if (argc != 4) { fprintf(stderr, "usage: mtpput <file> <folder> <name>\n"); return 2; }
  LIBMTP_Init();
  LIBMTP_mtpdevice_t *dev = LIBMTP_Get_First_Device();
  if (!dev) { fprintf(stderr, "no MTP device\n"); return 1; }
  LIBMTP_Get_Storage(dev, LIBMTP_STORAGE_SORTBY_NOTSORTED);
  for (LIBMTP_devicestorage_t *st = dev->storage; st; st = st->next) {
    LIBMTP_folder_t *folders = LIBMTP_Get_Folder_List_For_Storage(dev, st->id);
    uint32_t parent = (argv[2][0] >= '0' && argv[2][0] <= '9') ? (uint32_t)strtoul(argv[2], NULL, 10) : find(folders, argv[2]);
    if (!parent) continue;
    struct stat sb;
    if (stat(argv[1], &sb) != 0) { perror(argv[1]); return 1; }
    LIBMTP_file_t *file = LIBMTP_new_file_t();
    file->filename = strdup(argv[3]);
    file->filesize = sb.st_size;
    file->filetype = LIBMTP_FILETYPE_UNKNOWN;
    file->parent_id = parent;
    file->storage_id = st->id;
    printf("sending %s (%lld bytes) to %s/%s on \"%s\"\n", argv[1], (long long)sb.st_size, argv[2], argv[3], st->StorageDescription);
    int r = LIBMTP_Send_File_From_File(dev, argv[1], file, NULL, NULL);
    if (r != 0) { LIBMTP_Dump_Errorstack(dev); LIBMTP_Clear_Errorstack(dev); }
    else printf("done, object id %u\n", file->item_id);
    LIBMTP_Release_Device(dev);
    return r != 0;
  }
  fprintf(stderr, "folder %s not found\n", argv[2]);
  LIBMTP_Release_Device(dev);
  return 1;
}
