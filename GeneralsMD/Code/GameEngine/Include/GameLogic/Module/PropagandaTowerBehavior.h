// FILE: PropagandaTowerBehavior.h ////////////////////////////////////////////////////////////////
// Author: Colin Day, August 2002
// Desc:   Behavior module for PropagandaTower
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __PROPAGANDA_TOWER_BEHAVIOR_H_
#define __PROPAGANDA_TOWER_BEHAVIOR_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/BehaviorModule.h"
#include "GameLogic/Module/PropagandaTowerBehavior.h"
#include "GameLogic/Module/UpdateModule.h"
#include "GameLogic/Module/DieModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class ObjectTracker;
class FXList;
class UpgradeTemplate;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PropagandaTowerBehaviorModuleData: public UpdateModuleData
{

public:

	PropagandaTowerBehaviorModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	Real m_scanRadius;													///< radius of our scan
	UnsignedInt m_scanDelayInFrames;						///< how frequently we do an update scan
	Real m_autoHealPercentPerSecond;						///< how much % of max health we heal per second
	const FXList *m_pulseFX;										///< FXList to play when scan is updated
	AsciiString m_upgradeRequired;							///< Upgrade required to use the upgraded pulse FX
	Real m_upgradedAutoHealPercentPerSecond;		///< Different percent to use for healing if upgraded too
	const FXList *m_upgradedPulseFX;						///< FXList to play for pulse when upgraded
	Bool m_affectsSelf;													///< Allow effect to affect ourselves

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PropagandaTowerBehavior : public UpdateModule,
																public DieModuleInterface
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( PropagandaTowerBehavior, PropagandaTowerBehaviorModuleData );
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PropagandaTowerBehavior, "PropagandaTowerBehavior" )

public:

	PropagandaTowerBehavior( Thing *thing, const ModuleData *modData );
	// virtual destructor prototype provided by MemoryPoolObject

	// module methods
	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | (MODULEINTERFACE_DIE); }
	virtual void onDelete( void );
	void onObjectCreated( void );

	// update module methods
	virtual UpdateSleepTime update( void );

	// die module methods
	virtual DieModuleInterface *getDie( void ) { return this; }
	virtual void onDie( const DamageInfo *damageInfo );
	virtual void onCapture( Player *oldOwner, Player *newOwner );

	// Disabled conditions to process. Need to process when disabled, because our update needs to actively let go
	// of our effect on people.  We don't say "Be affected for n frames", we toggle people.  We need to process
	// so we can toggle everyone off.
	virtual DisabledMaskType getDisabledTypesToProcess() const { return DISABLEDMASK_ALL; }
	
	// our own public module methods

protected:

	virtual void removeAllInfluence( void );			///< remove any influence we had on all objects we've affected
	virtual void doScan( void );									///< do a scan
	virtual void effectLogic( Object *obj, Bool giving, 
														const PropagandaTowerBehaviorModuleData *modData);///< give/remove effect on object

	UnsignedInt m_lastScanFrame;									///< last frame we did a scan on

	ObjectTracker *m_insideList;									///< objects that are inside our area of influence
	const UpgradeTemplate *m_upgradeRequired;			///< Upgrade required to use the upgraded pulse FX

};

#endif  // end __PROPAGANDA_TOWER_BEHAVIOR_H_

