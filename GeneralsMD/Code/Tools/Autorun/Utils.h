#pragma once

#ifndef UTILS_H
#define UTILS_H

#include <windows.h>


/******************************************************************************
**	Swaps two objects.
*/
template<class T>
void swap( T & left, T & right )
{
	T temp;
   
	temp  = left;
	left  = right;
	right = temp;
}


void		Fix_Single_Ampersands  			( LPSTR pszString, bool upper_case );
void		Fix_Single_Ampersands  			( wchar_t *pszString, bool upper_case );
//UnicodeString Fix_Single_Ampersands ( UnicodeString string, bool upper_case);
void		Fix_Double_Ampersands  			( LPSTR string, bool upper_case );
void *		Load_Alloc_Data					( char *filename, long *filesize=0 );
void *		Load_File						( char *filename, long *filesize=0 );
char *		Make_Current_Path_To			( char *filename, char *path );
wchar_t *	Make_Current_Path_To			( wchar_t *filename, wchar_t *path );
char *		Path_Add_Back_Slash				( char *path );
char *		Path_Remove_Back_Slash			( char *path );
wchar_t *	Path_Add_Back_Slash				( wchar_t *path );
wchar_t *	Path_Remove_Back_Slash			( wchar_t *path );
void		PlugInProductName				( char *szString, int nName );
void		PlugInProductName				( char *szString, char *szName );
void		PlugInProductName				( wchar_t *szString, const wchar_t *szName );


#endif