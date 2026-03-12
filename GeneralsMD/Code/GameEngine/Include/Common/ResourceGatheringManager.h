// FILE: ResourceGatheringManager.h ///////////////////////////////////////////////////////////
// The part of a Player's brain that keeps track of all Resource type Objects and makes
// gathering type decisions based on them.
// Author: Graham Smallwood, January, 2002
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef RESOURCE_GATHER_MANAGER_H
#define RESOURCE_GATHER_MANAGER_H

#include "Common/GameType.h"
#include "Common/Snapshot.h"

class Object;

// ------------------------------------------------------------------------------------------------
class ResourceGatheringManager : public MemoryPoolObject,
																 public Snapshot
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ResourceGatheringManager, "ResourceGatheringManager" );

public:
	ResourceGatheringManager();

	Object *findBestSupplyWarehouse( Object *queryObject );		///< What Warehouse should this truck go to?
	Object *findBestSupplyCenter( Object *queryObject );			///< What Center should this truck return to?

	void addSupplyCenter( Object *newCenter );					///< I captured or built a Supply Center, so record it
	void removeSupplyCenter( Object *oldCenter );				///< Lost a supply center

	void addSupplyWarehouse( Object *newWarehouse );		///< Warehouse created, or this is starrt of game recording
	void removeSupplyWarehouse( Object *oldWarehouse );	///< Warehouse that doesn't replinish has run out of Supply

protected:

	// snapshot methods
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

private:
	/// @todo Make sure the allocator for std::list<> is a good one.  Otherwise override it.
	typedef std::list<ObjectID> objectIDList;
	typedef std::list<ObjectID>::iterator objectIDListIterator;

	objectIDList m_supplyWarehouses;
	objectIDList m_supplyCenters;

};

#endif