//
// loadsave.h
//

#ifndef __LOADSAVE_H
#define __LOADSAVE_H

int WriteMainDB(TransDB *db, const char *filename, CBabylonDlg *dlg );
int LoadMainDB(TransDB *db, const char *filename, void (*cb) (void ) = NULL );
int	GetLabelCountDB ( char *filename );


#endif