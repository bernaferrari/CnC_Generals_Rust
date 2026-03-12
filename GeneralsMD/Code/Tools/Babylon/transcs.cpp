//
// transcs.cpp
//

#include "stdafx.h"
#include <windows.h>
#include <winnls.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <memory.h>

void CreateTranslationTable ( void )
{
	int i; 
	FILE *out;
	unsigned short wc;
	unsigned short mb;
	DWORD last_error;

	if ( ! ( out = fopen ( "utable.c", "wt" )))
	{
		return;
	}


	fprintf (out, "static unsigned short Utable[0x10000] =\n{" );

	for ( i = 0; i < 0x10000; i++ )
	{

		if ( ( i %32 ) == 0 )
		{
			fprintf ( out, "\n\t/* 0x%04x */\t", i );
		}

		mb = i;
		if ( MultiByteToWideChar (CP_ACP, MB_ERR_INVALID_CHARS, (LPCSTR) &mb, 2, &wc, 2 ) == 0 )
		{
			wc = 0;
			last_error = GetLastError ( );
		}

		fprintf (out, "0x%04x", wc );
		if ( i != 0xffff )
		{
			fprintf (out, "," );
		}
	}

	fprintf ( out, "\n};\n");

	fclose ( out );

}
