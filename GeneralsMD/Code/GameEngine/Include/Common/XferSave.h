// FILE: XferSave.h ///////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   Xfer hard disk write implementation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __XFER_SAVE_H_
#define __XFER_SAVE_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "Common/Xfer.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class XferBlockData;
class Snapshot;

///////////////////////////////////////////////////////////////////////////////////////////////////
typedef long XferFilePos;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class XferSave : public Xfer
{

public:

	XferSave( void );
	virtual ~XferSave( void );

	// Xfer methods
	virtual void open( AsciiString identifier );		///< open file for writing
	virtual void close( void );											///< close file
	virtual Int beginBlock( void );									///< write placeholder block size
	virtual void endBlock( void );									///< backup to last begin block and write size
	virtual void skip( Int dataSize );							///< skipping during a write is a no-op

	virtual void xferSnapshot( Snapshot *snapshot );		///< entry point for xfering a snapshot

	// xfer methods
	virtual void xferAsciiString( AsciiString *asciiStringData );  ///< xfer ascii string (need our own)
	virtual void xferUnicodeString( UnicodeString *unicodeStringData );	///< xfer unicode string (need our own);

protected:

	virtual void xferImplementation( void *data, Int dataSize );		///< the xfer implementation

	FILE * m_fileFP;																			///< pointer to file
	XferBlockData *m_blockStack;													///< stack of block data

};

#endif // __XFER_SAVE_H_

