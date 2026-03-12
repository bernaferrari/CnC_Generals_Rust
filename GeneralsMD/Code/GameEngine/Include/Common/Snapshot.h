// FILE: Snapshot.h ///////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   The Snapshot object is the base class interface for data structures that will
//				 be considered during game saves, loads, and CRC checks.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SNAPSHOT_H_
#define __SNAPSHOT_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "Common/AsciiString.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Xfer;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class Snapshot
{

friend class GameState;
friend class XferLoad;
friend class XferSave;
friend class XferCRC;

public:
	
	Snapshot( void );
	~Snapshot( void );

protected:

	/// run the "light" crc check on this data structure
	virtual void crc( Xfer *xfer ) = 0;

	/** run save, load, or deep CRC check on this data structure, the type depends on the
	setup of the Xfer pointer */
	virtual void xfer( Xfer *xfer ) = 0;

	/** post process phase for loading save games.  All save systems have their xfer
	run using XferLoad mode, and then all systems each have their post process run */
	virtual void loadPostProcess( void ) = 0;

};

#endif // __SNAPSHOT_H_

