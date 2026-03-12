// FILE: FlammableUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, April 2002
// Desc:   Update that manages Aflame and Burned statuses and their effects
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FLAMMABLE_UPDATE_H_
#define __FLAMMABLE_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/AudioEventRTS.h"
#include "GameLogic/Module/DamageModule.h"
#include "GameLogic/Module/UpdateModule.h"


//-------------------------------------------------------------------------------------------------
enum FlammabilityStatusType
{
	// These show the state I last noticed my object was in.
	FS_NORMAL = 0,
	FS_AFLAME,
	FS_BURNED,

	FS_NORMAL_COUNT	// keep last
};

//-------------------------------------------------------------------------------------------------
class FlammableUpdateModuleData : public UpdateModuleData
{
public:
	UnsignedInt		m_burnedDelay;	///< How long before I am ::Burned.  0 means never
	UnsignedInt		m_aflameDuration; ///< How long I stay ::Aflame.  Independent of Burned.  
	// When aflame wears out is when I check to be normal or burned,  So my model can
	// change to burned while I am still aflame.
	UnsignedInt		m_aflameDamageDelay;	///< While ::Aflame, I take damage this often.  If 0, never.
	Int						m_aflameDamageAmount;	///< And this is how much I take.
	AsciiString		m_burningSoundName;			///< Sound to loop-play while burning (Not an AudioEventRTS here, since that belongs to the module)
	Real					m_flameDamageLimitData;
	UnsignedInt		m_flameDamageExpirationDelay;

	FlammableUpdateModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);

private:

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class FlammableUpdate : public UpdateModule, public DamageModuleInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FlammableUpdate, "FlammableUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FlammableUpdate, FlammableUpdateModuleData )

public:

	FlammableUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | (MODULEINTERFACE_DAMAGE); }
	virtual DamageModuleInterface* getDamage() { return this; }

	void tryToIgnite(); ///< FlammabeDamage uses this.  It is up to me to decide if I am burnable
	Bool wouldIgnite(); ///< Since we need to cheat sometimes and light something directly, ask if this would light

	//UpdateModuleInterface
	virtual UpdateSleepTime update();

	//DamageModuleInterface
	virtual void onDamage( DamageInfo *damageInfo );
	virtual void onHealing( DamageInfo *damageInfo ) { }
	virtual void onBodyDamageStateChange( const DamageInfo *damageInfo, 
																				BodyDamageType oldState, 
																				BodyDamageType newState ) { }

protected:
	
	UpdateSleepTime calcSleepTime();
	void doAflameDamage();
	void startBurningSound();
	void stopBurningSound();

	FlammabilityStatusType	m_status;
	UnsignedInt							m_aflameEndFrame;
	UnsignedInt							m_burnedEndFrame;
	UnsignedInt							m_damageEndFrame;
	AudioHandle							m_audioHandle;
	Real										m_flameDamageLimit;
	UnsignedInt							m_lastFlameDamageDealt;
};

#endif

