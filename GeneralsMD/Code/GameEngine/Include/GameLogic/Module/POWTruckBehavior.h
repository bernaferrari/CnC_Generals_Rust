// FILE: POWTruckBehavior.h ///////////////////////////////////////////////////////////////////////
// Author: Colin Day
// Desc:   POW Truck Behavior
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __POW_TRUCK_BEHAVIOR_H_
#define __POW_TRUCK_BEHAVIOR_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/OpenContain.h"

#ifdef ALLOW_SURRENDER

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class POWTruckBehaviorModuleData : public OpenContainModuleData
{

public:

	POWTruckBehaviorModuleData( void );
	
	static void buildFieldParse( MultiIniFieldParse &p );

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class POWTruckBehavior : public OpenContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( POWTruckBehavior, "POWTruckBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( POWTruckBehavior, POWTruckBehaviorModuleData )

public:

	POWTruckBehavior( Thing *thing, const ModuleData *moduleData );
	// virtual destructor prototype provided by memory pool declaration

	// collide methods
	virtual void onCollide( Object *other, const Coord3D *loc, const Coord3D *normal );
	
protected:

};

#endif

#endif  // end __POW_TRUCK_BEHAVIOR_H_
