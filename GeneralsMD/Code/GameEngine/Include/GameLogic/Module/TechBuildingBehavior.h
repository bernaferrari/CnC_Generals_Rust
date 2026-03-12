// FILE: TechBuildingBehavior.h ///////////////////////////////////////////////////////////////////
// Author: Colin Day, October 2002
// Desc:   Tech building basic behavior
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __TECH_BUILDING_BEHAVIOR_H_
#define __TECH_BUILDING_BEHAVIOR_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DieModule.h"
#include "GameLogic/Module/UpdateModule.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class TechBuildingBehaviorModuleData : public UpdateModuleData
{

public:

	TechBuildingBehaviorModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	const FXList*		m_pulseFX;										///< FXList to play when bldg is owned is updated
	UnsignedInt			m_pulseFXRate;								///< how frequently to play it

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class TechBuildingBehavior : public UpdateModule,
														 public DieModuleInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( TechBuildingBehavior, "TechBuildingBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( TechBuildingBehavior, TechBuildingBehaviorModuleData )

public:

	TechBuildingBehavior( Thing *thing, const ModuleData *modData );
	// virtual destructor prototype provided by memory pool object

	// module methods
	virtual void onCapture( Player *oldOwner, Player *newOwner );
	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | (MODULEINTERFACE_DIE); }

	// update methods
	virtual UpdateSleepTime update( void );

	// die methods
	virtual DieModuleInterface *getDie( void ) { return this; }
	virtual void onDie( const DamageInfo *damageInfo );

protected:

};

#endif  // end __TECH_BUILDING_BEHAVIOR_H_
