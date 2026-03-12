// FILE: BaikonurLaunchPower.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	November 2002
//
//	Filename: BaikonurLaunchPower.h
//
//	Author:		Kris Morness
//
//  Purpose:	Triggers the beginning of the launch for the baikonur launch tower.
//            This is used only by script to trigger the GLA end game.
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __BAIKONUR_LAUNCH_POWER_H_
#define __BAIKONUR_LAUNCH_POWER_H_

#include "GameLogic/Module/SpecialPowerModule.h"

class Object;
class SpecialPowerTemplate;
struct FieldParse;
enum ScienceType;

class BaikonurLaunchPowerModuleData : public SpecialPowerModuleData
{

public:

	BaikonurLaunchPowerModuleData( void );

	static void buildFieldParse( MultiIniFieldParse& p );

	AsciiString m_detonationObject;		
};


//-------------------------------------------------------------------------------------------------
class BaikonurLaunchPower : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( BaikonurLaunchPower, "BaikonurLaunchPower" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( BaikonurLaunchPower, BaikonurLaunchPowerModuleData )

public:

	BaikonurLaunchPower( Thing *thing, const ModuleData *moduleData );

	virtual void doSpecialPower( UnsignedInt commandOptions );
	virtual void doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions );

protected:

};

#endif // __BAIKONUR_LAUNCH_POWER_H_
