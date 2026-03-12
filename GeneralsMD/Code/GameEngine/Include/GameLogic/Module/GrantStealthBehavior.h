// FILE: GrantStealthBehavior.h /////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, June 2003
// Desc:   Update that grants permanent stealth to those whose stealth is permanently grantable
//------------------------------------------

#pragma once

#ifndef __GrantStealthBehavior_H_
#define __GrantStealthBehavior_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameClient/ParticleSys.h"
#include "GameLogic/Module/BehaviorModule.h"
#include "GameLogic/Module/UpgradeModule.h"
#include "GameLogic/Module/UpdateModule.h"
#include "GameLogic/Module/DamageModule.h"
#include "Common/BitFlagsIO.h"

class ParticleSystem;
class ParticleSystemTemplate;

//-------------------------------------------------------------------------------------------------
class GrantStealthBehaviorModuleData : public UpdateModuleData
{
public:
	Bool									m_initiallyActive;
	Bool									m_singleBurst;
	Int										m_healingAmount;
	Real									m_startRadius; 
	Real									m_finalRadius; 
	Real									m_radiusGrowRate; 
	KindOfMaskType				m_kindOf;	//Only these types can heal -- defaults to everything.
	const ParticleSystemTemplate*				m_radiusParticleSystemTmpl;					//Optional particle system meant to apply to entire effect for entire duration.

	GrantStealthBehaviorModuleData()
	{
		m_finalRadius = 200.0f;
		m_startRadius = 0.0f;
    m_radiusGrowRate = 10.0f;
		m_radiusParticleSystemTmpl = NULL;
		SET_ALL_KINDOFMASK_BITS( m_kindOf );
	}

	static void buildFieldParse( MultiIniFieldParse& p ) 
	{
		UpdateModuleData::buildFieldParse( p );
    
		static const FieldParse dataFieldParse[] = 
		{
			{ "StartRadius",						         INI::parseReal,									 NULL, offsetof( GrantStealthBehaviorModuleData, m_startRadius ) },
			{ "FinalRadius",						         INI::parseReal,									 NULL, offsetof( GrantStealthBehaviorModuleData, m_finalRadius ) },
			{ "RadiusGrowRate",						       INI::parseReal,									 NULL, offsetof( GrantStealthBehaviorModuleData, m_radiusGrowRate ) },
			{ "KindOf",						    KindOfMaskType::parseFromINI,					       NULL, offsetof( GrantStealthBehaviorModuleData, m_kindOf ) },		
			{ "RadiusParticleSystemName",				 INI::parseParticleSystemTemplate, NULL, offsetof( GrantStealthBehaviorModuleData, m_radiusParticleSystemTmpl ) },
			{ 0, 0, 0, 0 }
		};

		p.add(dataFieldParse);

  }

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class GrantStealthBehavior : public UpdateModule 
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( GrantStealthBehavior, "GrantStealthBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( GrantStealthBehavior, GrantStealthBehaviorModuleData )

public:

	GrantStealthBehavior( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();


private:

	void grantStealthToObject( Object *obj );
	ParticleSystemID m_radiusParticleSystemID;
  Real m_currentScanRadius;
};

#endif // __GrantStealthBehavior_H_

