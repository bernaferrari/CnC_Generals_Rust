// FILE: UnitCrateCollide.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that gives n units of type m the the pickerupper
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef UNIT_CRATE_COLLIDE_H_
#define UNIT_CRATE_COLLIDE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/Module.h"
#include "GameLogic/Module/CrateCollide.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class UnitCrateCollideModuleData : public CrateCollideModuleData
{
public:
	UnsignedInt m_unitCount;
	AsciiString m_unitType;

	UnitCrateCollideModuleData()
	{
		m_unitCount = 0;
		m_unitType = "";
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    CrateCollideModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "UnitCount",	INI::parseUnsignedInt,	NULL, offsetof( UnitCrateCollideModuleData, m_unitCount ) },
			{ "UnitName",		INI::parseAsciiString,	NULL, offsetof( UnitCrateCollideModuleData, m_unitType ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}
};

//-------------------------------------------------------------------------------------------------
class UnitCrateCollide : public CrateCollide
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( UnitCrateCollide, "UnitCrateCollide" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( UnitCrateCollide, UnitCrateCollideModuleData );

public:

	UnitCrateCollide( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// This is the game logic execution function that all real CrateCollides will implement
	virtual Bool executeCrateBehavior( Object *other );
protected:

};

#endif
