// FILE: ThreadUtils.cpp //////////////////////////////////////////////////////
// GameSpy thread utils
// Author: Matthew D. Campbell, July 2002

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

//-------------------------------------------------------------------------

std::wstring MultiByteToWideCharSingleLine( const char *orig )
{
	Int len = strlen(orig);
	WideChar *dest = NEW WideChar[len+1];

	MultiByteToWideChar(CP_UTF8, 0, orig, -1, dest, len);
	WideChar *c = NULL;
	do
	{
		c = wcschr(dest, L'\n');
		if (c)
		{
			*c = L' ';
		}
	}
	while ( c != NULL );
	do
	{
		c = wcschr(dest, L'\r');
		if (c)
		{
			*c = L' ';
		}
	}
	while ( c != NULL );

	dest[len] = 0;
	std::wstring ret = dest;
	delete dest;
	return ret;
}

std::string WideCharStringToMultiByte( const WideChar *orig )
{
	std::string ret;
	Int len = WideCharToMultiByte( CP_UTF8, 0, orig, wcslen(orig), NULL, 0, NULL, NULL ) + 1;
	if (len > 0)
	{
		char *dest = NEW char[len];
		WideCharToMultiByte( CP_UTF8, 0, orig, -1, dest, len, NULL, NULL );
		dest[len-1] = 0;
		ret = dest;
		delete dest;
	}
	return ret;
}

//-------------------------------------------------------------------------

