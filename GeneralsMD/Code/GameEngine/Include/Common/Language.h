// FILE: Language.h ///////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  Language.h
//
// Created:    Colin Day, June 2001
//
// Desc:       Header for dealing with multiple languages
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __LANGUAGE_H_
#define __LANGUAGE_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////

// FORWARD REFERENCES /////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// TYPE DEFINES ///////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// IMPORTANT: Make sure this enum is identical to the one in Noxstring tool
typedef enum
{

	LANGUAGE_ID_US = 0,
	LANGUAGE_ID_UK,
	LANGUAGE_ID_GERMAN,
	LANGUAGE_ID_FRENCH,
	LANGUAGE_ID_SPANISH,
	LANGUAGE_ID_ITALIAN,
	LANGUAGE_ID_JAPANESE,
	LANGUAGE_ID_JABBER,
	LANGUAGE_ID_KOREAN,
	LANGUAGE_ID_UNKNOWN

} LanguageID;

#define GameStrcpy wcscpy
#define GameStrncpy wcsncpy
#define GameStrlen wcslen
#define GameStrcat wcscat
#define GameStrcmp wcscmp
#define GameStrncmp wcsncmp
#define GameStricmp wcsicmp
#define GameStrnicmp wcsnicmp
#define GameStrtok wcstok
#define GameSprintf swprintf
#define GameVsprintf vswprintf
/// @todo -- add a non-malloc-based string dup func #define GameStrdup wcsdup
#define GameAtoi(S) wcstol( (S), NULL, 10)
#define GameAtod(S) wcstod( (S), NULL )
#define GameItoa _itow
#define GameSscanf swscanf
#define GameStrstr wcsstr 
#define GameStrchr wcschr
#define GameIsDigit iswdigit
#define GameIsAscii iswascii
#define GameIsAlNum iswalnum
#define GameIsAlpha iswalpha
#define GameArrayEnd(array) (array)[(sizeof(array)/sizeof((array)[0]))-1] = 0

// INLINING ///////////////////////////////////////////////////////////////////

// EXTERNALS //////////////////////////////////////////////////////////////////
extern LanguageID OurLanguage;  ///< our current language definition

#endif // __LANGUAGE_H_

