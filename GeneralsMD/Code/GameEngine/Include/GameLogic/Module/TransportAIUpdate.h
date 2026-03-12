// TransportAIUpdate.cpp //////////
// Needs to check legality of evacuate, and may move to a place that is better to evacuate at 
// Author: Graham Smallwood, July 2002
 
#pragma once

#ifndef _TRANSPORT_AI_UPDATE_H_
#define _TRANSPORT_AI_UPDATE_H_

#include "GameLogic/Module/AIUpdate.h"

//-------------------------------------------------------------------------------------------------
/** 
 * Soldier behavior implementation.
 * Override or extend AIUpdate methods to customize the Soldier's behavior.
 */
class TransportAIUpdate : public AIUpdateInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( TransportAIUpdate, "TransportAIUpdate" )
	MAKE_STANDARD_MODULE_MACRO( TransportAIUpdate )

	/*
		IMPORTANT NOTE: if you ever add module data to this, you must have it inherit from
		AIUpdateModuleData to allow locomotors to work correctly. (see SupplyTruckAIUpdate
		for an example.)
	*/

public:

	TransportAIUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration
	virtual AIFreeToExitType getAiFreeToExit(const Object* exiter) const;

protected:

	virtual AIStateMachine* makeStateMachine();

protected:
	virtual void privateAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );///< Extension.  Also tell occupants to attackObject
	virtual void privateAttackPosition( const Coord3D *pos, Int maxShotsToFire, CommandSourceType cmdSource );///< Extension.  Also tell occupants to attackPosition
	virtual void privateForceAttackObject( Object *victim, Int maxShotsToFire, CommandSourceType cmdSource );///< Extension.  Also tell occupants to forceAttackObject
};

#endif

