// FILE: CheckpointUpdate.h /////////////////////////////////////////////////////////////////////////////
// Author: 	Matthew D. Campbell, April 2002
// Desc:  Reacts when an enemy is within range
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef CHECKPOINT_UPDATE_H
#define CHECKPOINT_UPDATE_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
#include "Common/KindOf.h"


//-------------------------------------------------------------------------------------------------
/** Checkpoint update */
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
class CheckpointUpdateModuleData : public UpdateModuleData
{
public:
	UnsignedInt m_enemyScanDelayTime;

	CheckpointUpdateModuleData()
	{
		m_enemyScanDelayTime = LOGICFRAMES_PER_SECOND;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "ScanDelayTime",		INI::parseDurationUnsignedInt,		NULL, offsetof( CheckpointUpdateModuleData, m_enemyScanDelayTime ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class CheckpointUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CheckpointUpdate, "CheckpointUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CheckpointUpdate, CheckpointUpdateModuleData )

public:

	CheckpointUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

protected:
	Bool m_enemyNear;
	Bool m_allyNear;
	Real m_maxMinorRadius;

	UnsignedInt m_enemyScanDelay;
	void checkForAlliesAndEnemies( void );

};

#endif // end CHECKPOINT_UPDATE_H

