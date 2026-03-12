// FILE: CleanupAreaPower.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	September 2002
//
//	Author:		Kris Morness
//	
//  Makes use of the cleanup hazard update by augmenting the cleanup range 
//  until there is nothing left to cleanup at which time it goes idle.
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CLEANUP_AREA_POWER_H_
#define __CLEANUP_AREA_POWER_H_

//-----------------------------------------------------------------------------
#include "GameLogic/Module/SpecialPowerModule.h"

//-------------------------------------------------------------------------------------------------
class CleanupAreaPowerModuleData : public SpecialPowerModuleData
{

public:

	Real m_cleanupMoveRange;

	CleanupAreaPowerModuleData( void );
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
class CleanupAreaPower : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CleanupAreaPower, "CleanupAreaPower" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CleanupAreaPower, CleanupAreaPowerModuleData );

public:

	CleanupAreaPower( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	virtual void doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions );
};

#endif 
