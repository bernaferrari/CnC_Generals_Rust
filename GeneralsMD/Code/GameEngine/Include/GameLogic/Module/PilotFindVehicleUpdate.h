// FILE: PilotFindVehicleUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: John Ahlquist, Sept. 2002
// Desc:   Update module to handle wounded idle infantry finding a heal unit or structure.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __PILOT_FIND_VEHICLE_UPDATE_H_
#define __PILOT_FIND_VEHICLE_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/KindOf.h"
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ThingTemplate;
class WeaponTemplate;


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class PilotFindVehicleUpdateModuleData : public ModuleData
{
public:
	UnsignedInt			m_scanFrames;
	Real						m_scanRange;
	Real						m_minHealth;

	PilotFindVehicleUpdateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);

private: 

};

//-------------------------------------------------------------------------------------------------
/** The default	update module */
//-------------------------------------------------------------------------------------------------
class PilotFindVehicleUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PilotFindVehicleUpdate, "PilotFindVehicleUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( PilotFindVehicleUpdate, PilotFindVehicleUpdateModuleData );

public:

	PilotFindVehicleUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onObjectCreated();
	virtual UpdateSleepTime update();

	Object* scanClosestTarget();

protected:
	Bool		m_didMoveToBase;
};


#endif

