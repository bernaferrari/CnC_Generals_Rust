// FILE: Snapshot.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, February 2002
// Desc:   The Snapshot object is the base class interface for data structures that will
//				 be considered during game saves, loads, and CRC checks.
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/GameState.h"
#include "Common/Snapshot.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
Snapshot::Snapshot( void )
{

}  // end Snapshot

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
Snapshot::~Snapshot( void )
{

	//
	// if we're loading, there are pathological cases where we could destroy snapshots while
	// there is an entry for them in the post processing list ... need to clean this up
	//
	///@ todo, this might be needed in theory in the future, but iterating the post process
	// list in the game state is expensive because it's HUGE!
	//
//	TheGameState->notifySnapshotDeleted();

}  // end ~Snapshot
