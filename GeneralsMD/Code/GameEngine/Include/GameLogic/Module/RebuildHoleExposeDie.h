// FILE: RebuildHoleExposeDie.h ///////////////////////////////////////////////////////////////////
// Author: Colin Day, June 2002
// Desc:   When a structure dies with this module, a rebuild hole will be created in place
//				 of the structure that will automatically rebuild the structure
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __REBUILD_HOLE_EXPOSE_DIE_H_
#define __REBUILD_HOLE_EXPOSE_DIE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/INI.h"
#include "GameLogic/Module/DieModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class RebuildHoleExposeDieModuleData : public DieModuleData
{

public:

	RebuildHoleExposeDieModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	AsciiString m_holeName;			///< name of hole object to create
	Real m_holeMaxHealth;				///< max health of hole to create
	Bool m_transferAttackers;		///< if true, people attacking us should switch attacks to the hole when we die

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class RebuildHoleExposeDie : public DieModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RebuildHoleExposeDie, "RebuildHoleExposeDie"  )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( RebuildHoleExposeDie, RebuildHoleExposeDieModuleData );

public:

	RebuildHoleExposeDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onDie( const DamageInfo *damageInfo ); 

};


#endif  // end __REBUILD_HOLE_EXPOSE_DIE_H_
