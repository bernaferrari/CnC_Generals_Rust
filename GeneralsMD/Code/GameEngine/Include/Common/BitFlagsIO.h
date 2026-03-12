// FILE: BitFlagsIO.h /////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, March 2002
// Desc:   
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __BitFlagsIO_H_
#define __BitFlagsIO_H_

#include "Common/BitFlags.h"
#include "Common/INI.h"
#include "Common/Xfer.h"

//-------------------------------------------------------------------------------------------------

/*
template <size_t NUMBITS>
void BitFlags<NUMBITS>::buildDescription( AsciiString* str ) const
{
	if ( str == NULL )
		return;//sanity
 
	for( Int i = 0; i < size(); ++i )
	{
		const char* bitName = getBitNameIfSet(i);

		if (bitName != NULL)
		{
			str->concat( bitName );
			str->concat( ",\n");
		}
	}  
} 
*/

//-------------------------------------------------------------------------------------------------
template <size_t NUMBITS>
void BitFlags<NUMBITS>::parse(INI* ini, AsciiString* str)
{
//	m_bits.reset();
	if (str)
		str->clear();

	Bool foundNormal = false;
	Bool foundAddOrSub = false;

	// loop through all tokens
	for (const char *token = ini->getNextTokenOrNull(); token != NULL; token = ini->getNextTokenOrNull())
	{
		if (str)
		{
			if (str->isNotEmpty())
				str->concat(" ");
			str->concat(token);
		}

		if (stricmp(token, "NONE") == 0)
		{
			if (foundNormal || foundAddOrSub)
			{
				DEBUG_CRASH(("you may not mix normal and +- ops in bitstring lists"));
				throw INI_INVALID_NAME_LIST;
			}
			clear();
			break;
		}

		if (token[0] == '+')
		{
			if (foundNormal)
			{
				DEBUG_CRASH(("you may not mix normal and +- ops in bitstring lists"));
				throw INI_INVALID_NAME_LIST;
			}
			Int bitIndex = INI::scanIndexList(token+1, s_bitNameList);	// this throws if the token is not found
			set(bitIndex, 1);
			foundAddOrSub = true;
		}
		else if (token[0] == '-')
		{
			if (foundNormal)
			{
				DEBUG_CRASH(("you may not mix normal and +- ops in bitstring lists"));
				throw INI_INVALID_NAME_LIST;
			}
			Int bitIndex = INI::scanIndexList(token+1, s_bitNameList);	// this throws if the token is not found
			set(bitIndex, 0);
			foundAddOrSub = true;
		}
		else
		{
			if (foundAddOrSub)
			{
				DEBUG_CRASH(("you may not mix normal and +- ops in bitstring lists"));
				throw INI_INVALID_NAME_LIST;
			}

			if (!foundNormal)
				clear();

			Int bitIndex = INI::scanIndexList(token, s_bitNameList);	// this throws if the token is not found
			set(bitIndex, 1);
			foundNormal = true;
		}
	}
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
template <size_t NUMBITS>
/*static*/ void BitFlags<NUMBITS>::parseFromINI(INI* ini, void* /*instance*/, void *store, const void* /*userData*/)
{
	((BitFlags*)store)->parse(ini, NULL);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
template <size_t NUMBITS>
/*static*/ void BitFlags<NUMBITS>::parseSingleBitFromINI(INI* ini, void* /*instance*/, void *store, const void* /*userData*/)
{
	const char *token = ini->getNextToken();
	Int bitIndex = INI::scanIndexList(token, s_bitNameList);	// this throws if the token is not found

	Int *storeAsInt = (Int*)store;
	*storeAsInt = bitIndex;
}

//-------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
//-------------------------------------------------------------------------------------------------
template <size_t NUMBITS>
void BitFlags<NUMBITS>::xfer(Xfer* xfer)
{
	// this deserves a version number
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	if( xfer->getXferMode() == XFER_SAVE )
	{
		// save how many entries are to follow
		Int c = count();
		xfer->xferInt( &c );

		// save each of the string data
		for( Int i = 0; i < size(); ++i )
		{
			const char* bitName = getBitNameIfSet(i);

			// ignore if this kindof is not set in our mask data
			if (bitName == NULL)
				continue;

			// this bit is set, write the string value
			AsciiString bitNameA = bitName;
			xfer->xferAsciiString( &bitNameA );

		}  // end for i

	}  // end if, save
	else if( xfer->getXferMode() == XFER_LOAD )
	{
  	// clear the kind of mask data
		clear();

		// read how many entries follow
		Int c;
		xfer->xferInt( &c );

		// read each of the string entries
		AsciiString string;
		for( Int i = 0; i < c; ++i )
		{

			// read ascii string
			xfer->xferAsciiString( &string );

			// set in our mask type data
			Bool valid = setBitByName( string.str() );
			if (!valid)
			{
				DEBUG_CRASH(("invalid bit name %s",string.str()));
				throw XFER_READ_ERROR;
			}

		}  // end for, i

	}  // end else if, load
	else if( xfer->getXferMode() == XFER_CRC )
	{

		// just call the xfer implementation on the data values
		xfer->xferUser( this, sizeof( this ) );

	}  // end else if, crc
	else
	{

		DEBUG_CRASH(( "BitFlagsXfer - Unknown xfer mode '%d'\n", xfer->getXferMode() ));
		throw XFER_MODE_UNKNOWN;

	}  // end else

}  // end xfer

#endif
