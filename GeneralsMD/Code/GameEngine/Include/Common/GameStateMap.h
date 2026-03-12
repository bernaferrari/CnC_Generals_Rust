// FILE: GameStateMap.h ///////////////////////////////////////////////////////////////////////////
// Author: Colin Day, October 2002
// Desc:   Chunk in the save game file that will hold a pristine version of the map file
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GAME_STATE_MAP_H_
#define __GAME_STATE_MAP_H_

// INLCUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Snapshot.h"
#include "Common/SubsystemInterface.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Xfer;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class GameStateMap : public SubsystemInterface,
										 public Snapshot
{

public:

	GameStateMap( void );
	virtual ~GameStateMap( void );

	// subsystem interface methods
	virtual void init( void ) { }
	virtual void reset( void ) { }
	virtual void update( void ) { }

	// snapshot methods
	virtual void crc( Xfer *xfer ) { }
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void ) { }

	void clearScratchPadMaps( void );		///< clear any scratch pad maps from the save directory

protected:


};

// EXTERNALS //////////////////////////////////////////////////////////////////////////////////////
extern GameStateMap *TheGameStateMap;

#endif  // end __GAME_STATE_MAP_H_
