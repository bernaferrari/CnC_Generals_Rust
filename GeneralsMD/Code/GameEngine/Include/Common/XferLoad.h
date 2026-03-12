// FILE: XferLoad.h ///////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   Xfer hard disk read implementation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __XFER_LOAD_H_
#define __XFER_LOAD_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include <stdio.h>
#include "Common/Xfer.h"

// FOWARD REFERNCES ///////////////////////////////////////////////////////////////////////////////
class Snapshot;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class XferLoad : public Xfer
{

public:

	XferLoad( void );
	virtual ~XferLoad( void );

	virtual void open( AsciiString identifier );				///< open file for writing
	virtual void close( void );													///< close file
	virtual Int beginBlock( void );														///< read placeholder block size
	virtual void endBlock( void );											///< reading an end block is a no-op
	virtual void skip( Int dataSize );									///< skip forward dataSize bytes in file

	virtual void xferSnapshot( Snapshot *snapshot );		///< entry point for xfering a snapshot

	// xfer methods
	virtual void xferAsciiString( AsciiString *asciiStringData );  ///< xfer ascii string (need our own)
	virtual void xferUnicodeString( UnicodeString *unicodeStringData );	///< xfer unicode string (need our own);

protected:

	virtual void xferImplementation( void *data, Int dataSize );		///< the xfer implementation

	FILE * m_fileFP;																					///< pointer to file

};

#endif // __XFER_LOAD_H_

