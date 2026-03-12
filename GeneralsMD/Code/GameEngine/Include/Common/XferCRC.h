// FILE: XferCRC.h ////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   Xfer hard disk read implementation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __XFERCRC_H_
#define __XFERCRC_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "Common/Xfer.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Snapshot;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class XferCRC : public Xfer
{

public:

	XferCRC( void );
	virtual ~XferCRC( void );

	// Xfer methods
	virtual void open( AsciiString identifier );		///< start a CRC session with this xfer instance
	virtual void close( void );											///< stop CRC session
	virtual Int beginBlock( void );									///< start block event
	virtual void endBlock( void );									///< end block event
	virtual void skip( Int dataSize );							///< skip xfer event

	virtual void xferSnapshot( Snapshot *snapshot );		///< entry point for xfering a snapshot

	// Xfer CRC methods
	virtual UnsignedInt getCRC( void );										///< get computed CRC in network byte order

protected:

	virtual void xferImplementation( void *data, Int dataSize );

	void addCRC( UnsignedInt val );								///< CRC a 4-byte block

	UnsignedInt m_crc;

};

#endif // __XFERDISKWRITE_H_

