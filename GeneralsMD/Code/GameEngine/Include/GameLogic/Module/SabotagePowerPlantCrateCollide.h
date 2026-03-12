///////////////////////////////////////////////////////////////////////////////////////////////////
//	
// FILE: SabotagePowerPlantCrateCollide.h 
// Author: Kris Morness, June 2003
// Desc:   A crate (actually a saboteur - mobile crate) that makes the target powerplant lose power
//	
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef SABOTAGE_POWER_PLANT_CRATE_COLLIDE_H_
#define SABOTAGE_POWER_PLANT_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class SabotagePowerPlantCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_powerSabotageFrames;

	SabotagePowerPlantCrateCollideModuleData()
	{
		m_powerSabotageFrames = 0;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "SabotagePowerDuration", INI::parseDurationUnsignedInt, NULL, offsetof( SabotagePowerPlantCrateCollideModuleData, m_powerSabotageFrames ) },
			{ 0, 0, 0, 0 }
		};
		p.add( dataFieldParse );
	}

};

//-------------------------------------------------------------------------------------------------
class SabotagePowerPlantCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SabotagePowerPlantCrateCollide, "SabotagePowerPlantCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SabotagePowerPlantCrateCollide, SabotagePowerPlantCrateCollideModuleData );

public:

	SabotagePowerPlantCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	/// This allows specific vetoes to certain types of crates and their data
	virtual Bool isValidToExecute( const Object *other ) const;

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );

	virtual Bool isSabotageBuildingCrateCollide() const { return TRUE; }

};

#endif
