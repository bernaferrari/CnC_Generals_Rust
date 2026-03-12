// FILE: DamageModule.h /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2001
// Desc:	 
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DamageModule_H_
#define __DamageModule_H_

#include "Common/Module.h"
#include "GameLogic/Damage.h"
#include "GameLogic/Module/BehaviorModule.h"

enum BodyDamageType;

//-------------------------------------------------------------------------------------------------
/** OBJECT DAMAGE MODULE base class */
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
class DamageModuleInterface
{

public:

	virtual void onDamage( DamageInfo *damageInfo ) = 0;	///< damage callback
	virtual void onHealing( DamageInfo *damageInfo ) = 0;	///< healing callback
	virtual void onBodyDamageStateChange( const DamageInfo* damageInfo, 
																				BodyDamageType oldState, 
																				BodyDamageType newState) = 0;  ///< state change callback

};

//-------------------------------------------------------------------------------------------------
class DamageModuleData : public BehaviorModuleData
{
public:
//	DamageTypeFlags m_damageTypes;

	DamageModuleData()
//		: m_damageTypes(DAMAGE_TYPE_FLAGS_ALL)
	{
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    BehaviorModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
//			{ "DamageTypes", INI::parseDamageTypeFlags, NULL, offsetof( DamageModuleData, m_damageTypes ) },
			{ 0, 0, 0, 0 }
		};

    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
class DamageModule : public BehaviorModule, public DamageModuleInterface
{

	MEMORY_POOL_GLUE_ABC( DamageModule )
	MAKE_STANDARD_MODULE_MACRO_ABC( DamageModule )
	MAKE_STANDARD_MODULE_DATA_MACRO_ABC( DamageModule, DamageModuleData )

public:

	DamageModule( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	// module methods
	static Int getInterfaceMask() { return MODULEINTERFACE_DAMAGE; }

	// BehaviorModule
	virtual DamageModuleInterface* getDamage() { return this; }

	// damage module callbacks
	virtual void onDamage( DamageInfo *damageInfo ) = 0;	///< damage callback
	virtual void onHealing( DamageInfo *damageInfo ) = 0;	///< healing callback
	virtual void onBodyDamageStateChange( const DamageInfo* damageInfo, 
																				BodyDamageType oldState, 
																				BodyDamageType newState) = 0;  ///< state change callback

protected:

};
inline DamageModule::DamageModule( Thing *thing, const ModuleData* moduleData ) : BehaviorModule( thing, moduleData ) { }
inline DamageModule::~DamageModule() { }
//-------------------------------------------------------------------------------------------------

#endif
