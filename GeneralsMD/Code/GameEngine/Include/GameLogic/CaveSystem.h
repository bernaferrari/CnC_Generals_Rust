// FILE: CaveSystem.h /////////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood July 2002
// Desc:   System responsible for keeping track of the connectedness of all cave systems on the map
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef CAVE_SYSTEM_H
#define CAVE_SYSTEM_H

class Object;
class TunnelTracker; // The player owns one such object for his Tunnels, so instead of duplicating
// so much code, this SubSystem will manage all of the Cave systems.

#include "Common/Snapshot.h"
#include "Common/SubsystemInterface.h"

/** 
		System responsible for Crates as code objects - ini, new/delete etc
*/
class CaveSystem : public SubsystemInterface,
									 public Snapshot
{
public:
	CaveSystem();
	~CaveSystem();

	void init();
	void reset();
	void update();

	Bool canSwitchIndexToIndex( Int oldIndex, Int newIndex ); // If either Index has guys in it, no, you can't
	void registerNewCave( Int theIndex );			// All Caves are born with a default index, which could be new
	void unregisterCave( Int theIndex );				// 
	TunnelTracker *getTunnelTrackerForCaveIndex( Int theIndex );

protected:

	// snapshot methods
	virtual void crc( Xfer *xfer ) { }
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void ) { }

private:
	std::vector<TunnelTracker*> m_tunnelTrackerVector;// A vector of pointers where the indexes are known by
	// others, so it can have NULLs in it to keep position.  I've been advised against a map, so don't be a jerk
	// and use spot 20 first.

};

extern CaveSystem *TheCaveSystem;
#endif