// WanderAIUpdate.h //////////
// Will give self random move commands
// Author: Graham Smallwood, April 2002
 
#pragma once

#ifndef _WANDER_AI_UPDATE_H_
#define _WANDER_AI_UPDATE_H_

#include "GameLogic/Module/AIUpdate.h"

//-------------------------------------------------------------------------------------------------
/** 
 * Soldier behavior implementation.
 * Override or extend AIUpdate methods to customize the Soldier's behavior.
 */
class WanderAIUpdate : public AIUpdateInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( WanderAIUpdate, "WanderAIUpdate" )
	MAKE_STANDARD_MODULE_MACRO( WanderAIUpdate )

	/*
		IMPORTANT NOTE: if you ever add module data to this, you must have it inherit from
		AIUpdateModuleData to allow locomotors to work correctly. (see SupplyTruckAIUpdate
		for an example.)
	*/

	virtual UpdateSleepTime update();

public:

	WanderAIUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration


protected:

	virtual AIStateMachine* makeStateMachine();

};

#endif

