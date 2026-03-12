// FILE: DeletionUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August 2002
// Desc:   Update that will count down a lifetime and destroy object when it reaches zero
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DELETION_UPDATE_H_
#define __DELETION_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

//-------------------------------------------------------------------------------------------------
class DeletionUpdateModuleData : public UpdateModuleData
{
public:
	UnsignedInt m_minFrames;
	UnsignedInt m_maxFrames;

	DeletionUpdateModuleData()
	{
		m_minFrames = 0.0f;
		m_maxFrames = 0.0f;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "MinLifetime",					INI::parseDurationUnsignedInt,		NULL, offsetof( DeletionUpdateModuleData, m_minFrames ) },
			{ "MaxLifetime",					INI::parseDurationUnsignedInt,		NULL, offsetof( DeletionUpdateModuleData, m_maxFrames ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class DeletionUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DeletionUpdate, "DeletionUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DeletionUpdate, DeletionUpdateModuleData )

public:

	DeletionUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void setLifetimeRange( UnsignedInt minFrames, UnsignedInt maxFrames );
	UnsignedInt getDieFrame() { return m_dieFrame; }

	virtual UpdateSleepTime update( void );

protected:

	UnsignedInt calcSleepDelay(UnsignedInt minFrames, UnsignedInt maxFrames);

	UnsignedInt m_dieFrame;			///< frame we die on

};

#endif

