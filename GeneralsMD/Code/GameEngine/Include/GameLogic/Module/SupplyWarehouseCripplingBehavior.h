// FILE: SupplyWarehouseCripplingBehavior.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, Septemmber 2002
// Desc:   Behavior that Disables the building on ReallyDamaged edge state, and manages an Update timer to heal
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_H
#define _SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/BehaviorModule.h"
#include "GameLogic/Module/DamageModule.h"
#include "GameLogic/Module/UpdateModule.h"


//-------------------------------------------------------------------------------------------------
class SupplyWarehouseCripplingBehaviorModuleData : public UpdateModuleData
{
public:
	UnsignedInt m_selfHealSupression; ///< Time since last damage until I can start to heal
	UnsignedInt m_selfHealDelay;			///< Once I am okay to heal, how often to do so
	Real m_selfHealAmount;							///< And how much

	SupplyWarehouseCripplingBehaviorModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);

private:

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class SupplyWarehouseCripplingBehavior : public UpdateModule, 
																				 public DamageModuleInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SupplyWarehouseCripplingBehavior, "SupplyWarehouseCripplingBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SupplyWarehouseCripplingBehavior, SupplyWarehouseCripplingBehaviorModuleData )

public:

	SupplyWarehouseCripplingBehavior( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | (MODULEINTERFACE_DAMAGE); }

	// BehaviorModule
	virtual DamageModuleInterface* getDamage() { return this; }

	// DamageModuleInterface
	virtual void onDamage( DamageInfo *damageInfo );
	virtual void onHealing( DamageInfo *damageInfo ){}
	virtual void onBodyDamageStateChange(const DamageInfo* damageInfo, BodyDamageType oldState, BodyDamageType newState);

	// UpdateInterface
	virtual UpdateSleepTime update();

protected:
	virtual void resetSelfHealSupression();// Reset our able to heal timer, as we took damage
	virtual void startCrippledEffects();//Disable our object
	virtual void stopCrippledEffects();//Enable our object

private:
	UnsignedInt		m_healingSupressedUntilFrame;
	UnsignedInt		m_nextHealingFrame;
};

#endif

