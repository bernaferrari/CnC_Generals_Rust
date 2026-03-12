// FILE: VeterancyCrateCollide.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that gives a level of experience to all within n distance
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef VETERANCY_CRATE_COLLIDE_H_
#define VETERANCY_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class VeterancyCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_rangeOfEffect;
	Bool m_addsOwnerVeterancy;
	Bool m_isPilot;

	VeterancyCrateCollideModuleData()
	{
		m_rangeOfEffect = 0;
		m_addsOwnerVeterancy = false;
		m_isPilot = false;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "EffectRange",	INI::parseUnsignedInt,	NULL, offsetof( VeterancyCrateCollideModuleData, m_rangeOfEffect ) },
			{ "AddsOwnerVeterancy",	INI::parseBool,	NULL, offsetof( VeterancyCrateCollideModuleData, m_addsOwnerVeterancy ) },
			{ "IsPilot", INI::parseBool, NULL, offsetof( VeterancyCrateCollideModuleData, m_isPilot ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}
};

//-------------------------------------------------------------------------------------------------
class VeterancyCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( VeterancyCrateCollide, "VeterancyCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( VeterancyCrateCollide, VeterancyCrateCollideModuleData );

public:

	VeterancyCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This allows specific vetoes to certain types of crates and their data
	virtual Bool isValidToExecute( const Object *other ) const;

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	Int getLevelsToGain() const;

};

#endif
