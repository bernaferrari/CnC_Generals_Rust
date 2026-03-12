// FILE: ProjectileStreamUpdate.h //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, May 2002
// Desc:   Tracks all projectiles fired so they can be drawn as a stream
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __PROJECTILE_STREAM_UPDATE_H_
#define __PROJECTILE_STREAM_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class Vector3;

enum
{
	MAX_PROJECTILE_STREAM = 20
};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class ProjectileStreamUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ProjectileStreamUpdate, "ProjectileStreamUpdate" )
	MAKE_STANDARD_MODULE_MACRO( ProjectileStreamUpdate );

public:

	ProjectileStreamUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void addProjectile( ObjectID sourceID, ObjectID newID, ObjectID victimID, const Coord3D *victimPos );	///< This projectile was just shot, so keep track of it.
	void getAllPoints( Vector3 *points, Int *count );					///< unroll circlular array and write down all projectile positions
	void setPosition( const Coord3D *newPosition );						///< I need to exist at the place I want to draw since only (near) on screen Drawables get updated

	virtual UpdateSleepTime update();

protected:


	void cullFrontOfList();
	Bool considerDying();

	ObjectID m_projectileIDs[MAX_PROJECTILE_STREAM];
	Int m_nextFreeIndex;
	Int m_firstValidIndex;
	ObjectID m_owningObject;
	
	ObjectID m_targetObject;///< Need to insert a hole if target changes, so track target ID and target position
	Coord3D m_targetPosition;
};


#endif

