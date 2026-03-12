// FILE: TunnelTracker.h ///////////////////////////////////////////////////////////
// The part of a Player's brain that holds the communal Passenger list of all tunnels.
// This has a similar interface to a ContainModule, naturally, but players can't have modules.
// Author: Graham Smallwood, March, 2002

#pragma once

#ifndef TUNNEL_TRACKER_H
#define TUNNEL_TRACKER_H

#include "Common/GameType.h"
#include "Common/GameMemory.h"
#include "Common/Snapshot.h"
#include "GameLogic/Module/ContainModule.h"

class TunnelTracker : public MemoryPoolObject,
											public Snapshot
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( TunnelTracker, "TunnelTracker" );

public:
	TunnelTracker();
	// contain list access
	void iterateContained( ContainIterateFunc func, void *userData, Bool reverse );
	UnsignedInt getContainCount() const { return m_containListSize; }
	Int getContainMax() const;
	const ContainedItemsList* getContainedItemsList() const { return &m_containList; }	

	Bool isValidContainerFor(const Object* obj, Bool checkCapacity) const;
	void addToContainList( Object *obj );				///< add 'obj' to contain list
	void removeFromContain( Object *obj, Bool exposeStealthUnits = FALSE );	///< remove 'obj' from contain list
	Bool isInContainer( Object *obj );				///< Is this thing inside?

	void onTunnelCreated( const Object *newTunnel );		///< A tunnel was made
	void onTunnelDestroyed( const Object *deadTunnel );	///< A tunnel was destroyed

	static void destroyObject( Object *obj, void *userData ); ///< Callback for Iterate Contained system
	static void healObject( Object *obj, void *frames ); ///< Callback for Iterate Contained system

	void healObjects(Real frames);	///< heal all objects within the tunnel
	
	UnsignedInt friend_getTunnelCount() const {return m_tunnelCount;}///< TunnelContains are allowed to ask if they are the last one ahead of deletion time

	const std::list< ObjectID > *getContainerList() const {return &m_tunnelIDs;}

	Object *getCurNemesis(void);
	void updateNemesis(const Object *target);

protected:

	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

private:
	
	std::list< ObjectID > m_tunnelIDs;			///< I have to try to keep track of these because Caves need to iterate on them.
	ContainedItemsList m_containList;				///< the contained object pointers list
	std::list< ObjectID > m_xferContainList;///< for loading of m_containList during post processing
	Int m_containListSize;									///< size of the contain list
	UnsignedInt m_tunnelCount;							///< How many tunnels have registered so we know when we should kill our contain list

	ObjectID		m_curNemesisID;							///< If we have team(s) guarding a tunnel network system, this is one of the current targets.
	UnsignedInt m_nemesisTimestamp;					///< We only keep nemesis for a couple of seconds.
};

#endif
