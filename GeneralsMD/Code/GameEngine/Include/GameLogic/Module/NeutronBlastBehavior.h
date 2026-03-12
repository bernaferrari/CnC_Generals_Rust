// FILE: NeutronBlastBehavior.h /////////////////////////////////////////////////////////////////////////
// Author: Daniel Teh, July 2003
// Desc:   Create a neutron blast behavior that wipes out infantry, no matter where they hide
//------------------------------------------

#pragma once

#ifndef __NeutronBlastBehavior_H_
#define __NeutronBlastBehavior_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DieModule.h"
#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
class NeutronBlastBehaviorModuleData : public UpdateModuleData
{
public:
	Real m_blastRadius; 
	Bool m_isAffectAirborne;
	Bool m_affectAllies;

	NeutronBlastBehaviorModuleData()
	{
		m_blastRadius = 10.0f;
		m_isAffectAirborne = TRUE;
		m_affectAllies = TRUE;
	}

	static void buildFieldParse( MultiIniFieldParse& p ) 
	{
		UpdateModuleData::buildFieldParse( p );
    
		static const FieldParse dataFieldParse[] = 
		{
			{ "BlastRadius",		INI::parseReal, NULL, offsetof( NeutronBlastBehaviorModuleData, m_blastRadius ) },
			{ "AffectAirborne", INI::parseBool, NULL, offsetof( NeutronBlastBehaviorModuleData, m_isAffectAirborne ) },
			{ "AffectAllies",		INI::parseBool, NULL, offsetof( NeutronBlastBehaviorModuleData, m_affectAllies ) },
			{ 0, 0, 0, 0 }
		};

		p.add(dataFieldParse);
  }

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class NeutronBlastBehavior : public UpdateModule,
														 public DieModuleInterface 
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( NeutronBlastBehavior, "NeutronBlastBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( NeutronBlastBehavior, NeutronBlastBehaviorModuleData )

public:

	NeutronBlastBehavior( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	static Int getInterfaceMask() { return UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE; }
	virtual DieModuleInterface* getDie() { return this; }

	
	virtual UpdateSleepTime update();
	virtual void onDie( const DamageInfo *damageInfo );


private:

	void neutronBlastToObject( Object *obj );
};

#endif // __NeutronBlastBehavior_H_

