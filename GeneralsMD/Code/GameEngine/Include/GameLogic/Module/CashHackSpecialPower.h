// FILE: CashHackSpecialPower.h ///////////////////////////////////////////////////////////////
// Author: Colin Day, June 2002
// Desc:   The Spy Satellite will reveal shrouded areas of the map that the player chooses
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __CASHHACKSPECIALPOWER_H_
#define __CASHHACKSPECIALPOWER_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/SpecialPowerModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;
class SpecialPowerTemplate;
struct FieldParse;
enum ScienceType;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class CashHackSpecialPowerModuleData : public SpecialPowerModuleData
{

public:

	struct Upgrades
	{
		ScienceType									m_science;
		Int													m_amountToSteal;

		Upgrades() : m_science(SCIENCE_INVALID), m_amountToSteal(0)
		{
		}
	};
	std::vector<Upgrades> m_upgrades;
	Int m_defaultAmountToSteal;					///< the amount of money that we will steal

	CashHackSpecialPowerModuleData( void );
	static void buildFieldParse( MultiIniFieldParse& p );
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class CashHackSpecialPower : public SpecialPowerModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CashHackSpecialPower, "CashHackSpecialPower" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CashHackSpecialPower, CashHackSpecialPowerModuleData )

public:

	CashHackSpecialPower( Thing *thing, const ModuleData *moduleData );
	// virtual destructor provided by memory pool object

	virtual void doSpecialPowerAtObject( Object *obj, UnsignedInt commandOptions );
	virtual void doSpecialPowerAtLocation( const Coord3D *loc, Real angle, UnsignedInt commandOptions );

protected:

	Int findAmountToSteal() const;

};

#endif  // end __CASHHACKSPECIALPOWER_H_

