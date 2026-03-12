// FILE: BaseRegenerateUpdate.h ///////////////////////////////////////////////////////////////////
// Author: Colin Day, July 2002
// Desc:   Update module for base objects automatically regenerating health
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __BASE_REGENERATE_UPDATE_H_
#define __BASE_REGENERATE_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
#include "GameLogic/Module/DamageModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class BaseRegenerateUpdateModuleData : public UpdateModuleData
{

public:
	BaseRegenerateUpdateModuleData( void );
	static void buildFieldParse( MultiIniFieldParse &p );

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class BaseRegenerateUpdate : public UpdateModule,
												 public DamageModuleInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( BaseRegenerateUpdate, "BaseRegenerateUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( BaseRegenerateUpdate, BaseRegenerateUpdateModuleData );

public:

	BaseRegenerateUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | MODULEINTERFACE_DAMAGE; }

	// BehaviorModule
	virtual DamageModuleInterface* getDamage() { return this; }

	// UpdateModuleInterface
	virtual UpdateSleepTime update( void );

	// DamageModuleInterface
	virtual void onDamage( DamageInfo *damageInfo );
	virtual void onHealing( DamageInfo *damageInfo ) { }
	virtual void onBodyDamageStateChange(const DamageInfo* damageInfo, BodyDamageType oldState, BodyDamageType newState) { }
	virtual DisabledMaskType getDisabledTypesToProcess() const { return MAKE_DISABLED_MASK( DISABLED_UNDERPOWERED ); }

private:

};

#endif  // end __BASE_REGENERATE_UPDATE_H_
