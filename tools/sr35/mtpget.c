// mtpget <out dir> <name>...: fetch files by name (first match) in one session, print parent folder ids
#include <libmtp.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
  LIBMTP_Init();
  LIBMTP_mtpdevice_t *dev = LIBMTP_Get_First_Device();
  if (!dev) return 1;
  LIBMTP_file_t *all = LIBMTP_Get_Filelisting_With_Callback(dev, NULL, NULL);
  LIBMTP_folder_t *folders = LIBMTP_Get_Folder_List(dev);
  for (int i = 2; i < argc; i++) {
    for (LIBMTP_file_t *f = all; f; f = f->next) if (strcmp(f->filename, argv[i]) == 0) {
      char out[2048]; snprintf(out, sizeof out, "%s/%s", argv[1], argv[i]);
      int r = LIBMTP_Get_File_To_File(dev, f->item_id, out, NULL, NULL);
      LIBMTP_folder_t *p = LIBMTP_Find_Folder(folders, f->parent_id);
      printf("%s -> %d, storage %08x, parent %s\n", argv[i], r, f->storage_id, p ? p->name : "?");
      break;
    }
  }
  LIBMTP_Release_Device(dev);
}
