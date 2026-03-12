// FILE: DefaultProductionExitUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, January, 2002
// Desc:		Hand off produced Units to me so I can Exit them into the world with my specific style
//					This instance simply spits the guy out at a point.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _DEFAULT_PRODUCTION_EXIT_UPDATE_H
#define _DEFAULT_PRODUCTION_EXIT_UPDATE_H

#include "GameLogic/Module/UpdateModule.h"
#include "Common/INI.h"
#include "Lib/BaseType.h"

class Object;

//-------------------------------------------------------------------------------------------------
class DefaultProductionExitUpdateModuleData : public UpdateModuleData
{
public:
	Coord3D m_unitCreatePoint;
	Coord3D m_naturalRallyPoint;
	Bool		m_useSpawnRallyPoint;

	DefaultProductionExitUpdateModuleData()
	{
		m_unitCreatePoint.zero();
		m_naturalRallyPoint.zero();
		m_useSpawnRallyPoint = false;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "UnitCreatePoint",		INI::parseCoord3D,		NULL, offsetof( DefaultProductionExitUpdateModuleData, m_unitCreatePoint ) },
			{ "NaturalRallyPoint",  INI::parseCoord3D,		NULL, offsetof( DefaultProductionExitUpdateModuleData, m_naturalRallyPoint ) },
			{ "UseSpawnRallyPoint", INI::parseBool,				NULL, offsetof( DefaultProductionExitUpdateModuleData, m_useSpawnRallyPoint ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
class DefaultProductionExitUpdate : public UpdateModule, public ExitInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DefaultProductionExitUpdate, "DefaultProductionExitUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DefaultProductionExitUpdate, DefaultProductionExitUpdateModuleData )

public:

	virtual ExitInterface* getUpdateExitInterface() { return this; }

	DefaultProductionExitUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	// Required funcs to fufill interface requirements
	virtual Bool isExitBusy() const {return FALSE;}	///< Contain style exiters are getting the ability to space out exits, so ask this before reserveDoor as a kind of no-commitment check.
	virtual ExitDoorType reserveDoorForExit( const ThingTemplate* objType, Object *specificObject ) { return DOOR_1; }
	virtual void exitObjectViaDoor( Object *newObj, ExitDoorType exitDoor );
	virtual void unreserveDoorForExit( ExitDoorType exitDoor ) { /* nothing */ }
	virtual void exitObjectByBudding( Object *newObj, Object *budHost ) { return; }

	virtual void setRallyPoint( const Coord3D *pos );				///< define a "rally point" for units to move towards
	virtual const Coord3D *getRallyPoint( void ) const;			///< define a "rally point" for units to move towards
	virtual Bool useSpawnRallyPoint( void ) const;
	virtual Bool getNaturalRallyPoint( Coord3D& rallyPoint, Bool offset = TRUE ) const;			///< get the natural "rally point" for units to move towards
	virtual Bool getExitPosition( Coord3D& exitPosition ) const;					///< access to the "Door" position of the production object
	virtual UpdateSleepTime update()										{ return UPDATE_SLEEP_FOREVER; }

protected:

	Coord3D m_rallyPoint;						///< Where units should move to after they have reached the "natural" rally point
	Bool m_rallyPointExists;				///< Only move to the rally point if this is true

};

//-------------------------------------------------------------------------------------------------
inline void DefaultProductionExitUpdate::setRallyPoint( const Coord3D *pos )
{
	m_rallyPoint = *pos;
	m_rallyPointExists = true;
}

//-------------------------------------------------------------------------------------------------
inline const Coord3D *DefaultProductionExitUpdate::getRallyPoint( void ) const
{
	if (m_rallyPointExists)
		return &m_rallyPoint;

	return NULL;
}

//-------------------------------------------------------------------------------------------------
inline Bool DefaultProductionExitUpdate::useSpawnRallyPoint( void ) const
{
	// Check if the building has requested spawn units (like those that are airdropped)
	// to use the rally points of the building.
	if (getDefaultProductionExitUpdateModuleData()->m_useSpawnRallyPoint)
		return TRUE;
	else
		return FALSE;
}

#endif
