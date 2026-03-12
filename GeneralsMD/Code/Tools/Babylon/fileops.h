//
// File IO support
//

#ifndef __FILEOPS_H
#define __FILEOPS_H

static const int FA_NOFILE = 0;
static const int 	FA_READONLY = 0x00000001;
static const int 	FA_DIRECTORY = 0x00000002;
static const int 	FA_WRITEABLE = 0x00000004;


int							FileExists ( const char *filename );
int					 		FileAttribs ( const char *filename );
void						MakeBackupFile ( const char *filename );
void						RestoreBackupFile ( const char *filename );


#endif		// __FILEIO_H