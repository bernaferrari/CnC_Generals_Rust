///////////////////////////////////////////////////////////////////////////////////////////////////
//	
// FILE: SabotageInternetCenterCrateCollide.h 
// Author: Kris Morness, July 2003
// Desc:   A crate (actually a saboteur - mobile crate) that temporarily disables an internet center
//	
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef SABOTAGE_INTERNET_CENTER_CRATE_COLLIDE_H_
#define SABOTAGE_INTERNET_CENTER_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class SabotageInternetCenterCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_sabotageFrames;

	SabotageInternetCenterCrateCollideModuleData()
	{
		m_sabotageFrames = 0;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "SabotageDuration", INI::parseDurationUnsignedInt, NULL, offsetof( SabotageInternetCenterCrateCollideModuleData, m_sabotageFrames ) },
			{ 0, 0, 0, 0 }
		};
		p.add( dataFieldParse );
	}

};

//-------------------------------------------------------------------------------------------------
class SabotageInternetCenterCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SabotageInternetCenterCrateCollide, "SabotageInternetCenterCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SabotageInternetCenterCrateCollide, SabotageInternetCenterCrateCollideModuleData );

public:

	SabotageInternetCenterCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This allows specific vetoes to certain types of crates and their data
	virtual Bool isValidToExecute( const Object *other ) const;

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	virtual Bool isSabotageBuildingCrateCollide() const { return TRUE; }

};

#endif
