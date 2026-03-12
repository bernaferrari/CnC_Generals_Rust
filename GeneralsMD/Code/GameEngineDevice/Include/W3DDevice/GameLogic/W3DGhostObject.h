// FILE: W3DGhostObject.h ////////////////////////////////////////////////////////////
// Placeholder for objects that have been deleted but need to be maintained because
// a player can see them fogged.
// Author: Mark Wilczynski, August 2002

#pragma once

#ifndef _W3DGHOSTOBJECT_H_
#define _W3DGHOSTOBJECT_H_

#include "GameLogic/GhostObject.h"
#include "Lib/BaseType.h"
#include "Common/GameCommon.h"
#include "GameClient/DrawableInfo.h"

class Object;
class W3DGhostObjectManager;
class W3DRenderObjectSnapshot;
class PartitionData;

class W3DGhostObject: public GhostObject
{
	friend W3DGhostObjectManager;
public:
	W3DGhostObject();
	virtual ~W3DGhostObject();
	virtual void snapShot(int playerIndex);
	virtual void updateParentObject(Object *object, PartitionData *mod);
	virtual void freeSnapShot(int playerIndex);
protected:
	virtual void crc( Xfer *xfer);
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );
	void removeParentObject(void);
	void restoreParentObject(void);	///< restore the original non-ghosted object to scene.
	void addToScene(int playerIndex);
	void removeFromScene(int playerIndex);
	void release(void);			///< used by manager to return object to free store.
	void getShroudStatus(int playerIndex);	///< used to get the partition manager to update ghost objects without parent objects.
	void freeAllSnapShots(void);				///< used to free all snapshots from all players.
	W3DRenderObjectSnapshot *m_parentSnapshots[MAX_PLAYER_COUNT];
	DrawableInfo	m_drawableInfo;

	///@todo this list should really be part of the device independent base class (CBD 12-3-2002)
	W3DGhostObject *m_nextSystem;
	W3DGhostObject *m_prevSystem;
};

class W3DGhostObjectManager : public GhostObjectManager
{
public:
	W3DGhostObjectManager();
	virtual ~W3DGhostObjectManager();
	virtual void reset(void);
	virtual GhostObject *addGhostObject(Object *object, PartitionData *pd);
	virtual void removeGhostObject(GhostObject *mod);
	virtual void setLocalPlayerIndex(int index);
	virtual void updateOrphanedObjects(int *playerIndexList, int numNonLocalPlayers);
	virtual void W3DGhostObjectManager::releasePartitionData(void);
	virtual void W3DGhostObjectManager::restorePartitionData(void);

protected:
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

	///@todo this list should really be part of the device independent base class (CBD 12-3-2002)
	W3DGhostObject	*m_freeModules;
	W3DGhostObject	*m_usedModules;
};

#endif // _W3DGHOSTOBJECT_H_
