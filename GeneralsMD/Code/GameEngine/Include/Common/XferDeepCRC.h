// FILE: XferDeepCRC.h ////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   Xfer hard disk read implementation
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __XFERDEEPCRC_H_
#define __XFERDEEPCRC_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "Common/Xfer.h"
#include "Common/XferCRC.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Snapshot;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class XferDeepCRC : public XferCRC
{

public:

	XferDeepCRC( void );
	virtual ~XferDeepCRC( void );

	// Xfer methods
	virtual void open( AsciiString identifier );		///< start a CRC session with this xfer instance
	virtual void close( void );											///< stop CRC session

	// xfer methods
	virtual void xferMarkerLabel( AsciiString asciiStringData );  ///< xfer ascii string (need our own)
	virtual void xferAsciiString( AsciiString *asciiStringData );  ///< xfer ascii string (need our own)
	virtual void xferUnicodeString( UnicodeString *unicodeStringData );	///< xfer unicode string (need our own);

protected:

	virtual void xferImplementation( void *data, Int dataSize );

	FILE * m_fileFP;																			///< pointer to file
};

#endif // __XFERDEEPCRC_H_

