//
// expimp.h
//

#ifndef __EXPIMP_H
#define __EXPIMP_H

#include "transDB.h"
#include "Babylondlg.h"

typedef enum
{
	TR_ALL,
	TR_CHANGES,
	TR_DIALOG,
	TR_NONDIALOG,
	TR_SAMPLE,
	TR_MISSING_DIALOG,
	TR_UNVERIFIED,
	TR_UNSENT
} TrFilter;

typedef struct 
{
	TrFilter filter;
	int include_comments;
	int include_translations;

} TROPTIONS;

typedef enum
{
	GN_UNICODE,
	GN_BABYLONSTR,
} GnFormat;

typedef enum
{
	GN_USEIDS,
	GN_USEORIGINAL,
} GnUntranslated;

typedef struct 
{
	GnFormat	format;								// what file format to generate
	GnUntranslated untranslated;		// what to do with untranslated text

} GNOPTIONS;

typedef struct 
{
	int translations;
	int dialog;
	int limit;

} RPOPTIONS;


#define CSF_ID ( ('C'<<24) | ('S'<<16) | ('F'<<8) | (' ') )
#define CSF_LABEL ( ('L'<<24) | ('B'<<16) | ('L'<<8) | (' ') )
#define CSF_STRING ( ('S'<<24) | ('T'<<16) | ('R'<<8) | (' ') )
#define CSF_STRINGWITHWAVE ( ('S'<<24) | ('T'<<16) | ('R'<<8) | ('W') )
#define CSF_VERSION 3

typedef struct
{
	int id;
	int version;
	int num_labels;
	int num_strings;
	int skip;	

} CSF_HEADER_V1;

typedef struct
{
	int id;
	int version;
	int num_labels;
	int num_strings;
	int skip;	
	int langid;

} CSF_HEADER;

int ExportTranslations ( TransDB *db, const char *filename, LangID langid, TROPTIONS *options, CBabylonDlg *dlg = NULL );
int ImportTranslations ( TransDB *db, const char *filename, CBabylonDlg *dlg = NULL );
int UpdateSentTranslations ( TransDB *db, const char *filename, CBabylonDlg *dlg = NULL );
int GenerateGameFiles ( TransDB *db, const char *filename, GNOPTIONS *option, LangID *languages, CBabylonDlg *dlg = NULL );
int GenerateReport ( TransDB *db, const char *filename, RPOPTIONS *options, LangID *languages, CBabylonDlg *dlg = NULL );
void ProcessWaves ( TransDB *db, const char *filename, CBabylonDlg *dlg );
#endif