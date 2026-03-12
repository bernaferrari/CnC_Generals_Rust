///////////////////////////////////////////////////////////////////////////////////////////////////
//
// FILE: DefectorSpecialPower.h 
// Author: Mark Lorenzen, JULY 2002
// Desc:   General can click command cursor on any enemy, and it becomes his
//
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DEFECTORSPECIALPOWER_H_
#define __DEFECTORSPECIALPOWER_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/SpecialPowerModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;
class SpecialPowerTemplate;
struct FieldParse;
enum ScienceType;




//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class DefectorSpecialPowerModuleData : public SpecialPowerModuleData
{

public:

	DefectorSpecialPowerModuleData( void );

	static void buildFieldParse( MultiIniFieldParse& p );

	Real m_fatCursorRadius;					///< the distance around the target we will reveal

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class DefectorSpecialPower : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DefectorSpecialPower, "DefectorSpecialPower" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DefectorSpecialPower, DefectorSpecialPowerModuleData )

public:

	DefectorSpecialPower( Thing *thing, const ModuleData *moduleData );
	// virtual destructor prototype provided by memory pool object

	virtual void doSpecialPowerAtObject( Object *obj, UnsignedInt commandOptions );
	virtual void doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions );

protected:

};
#endif  // end DefectorSpecialPower

