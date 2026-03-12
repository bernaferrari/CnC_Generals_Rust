#pragma once

#ifndef  LOCALE_API_H
#define  LOCALE_API_H

#include <STDLIB.H>

/****************************************************************************/
/* GLOBAL VARIABLES                                                         */
/****************************************************************************/
extern	int		CodePage;
extern	void *	LocaleFile;
extern	int		LanguageID;
extern	char	LanguageFile[];
extern	int		PrimaryLanguage;
extern	int		SubLanguage;

/****************************************************************************/
/* LOCALE API                                                               */
/****************************************************************************/
int				Locale_Init						( int language, char *file );
void			Locale_Restore					( void );
const wchar_t* Locale_GetString( const char *id, wchar_t *buffer = NULL, int size = _MAX_PATH );
/*
const char*		Locale_GetString				( int StringID, char *String );
const wchar_t*	Locale_GetString				( int StringID, wchar_t *String=NULL );
*/
bool			Locale_Use_Multi_Language_Files	( void );
//int				Locale_Get_Language_ID 			( void )	{ return LanguageID; };

#endif