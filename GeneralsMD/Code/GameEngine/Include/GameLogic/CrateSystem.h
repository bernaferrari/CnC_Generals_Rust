// FILE: CrateSystem.h /////////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood Feb 2002
// Desc:   System responsible for Crates as code objects - ini, new/delete etc
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef CRATE_SYSTEM_H
#define CRATE_SYSTEM_H

#include "Common/Ini.h"
#include "Common/Overridable.h"
#include "Common/Override.h"

enum ScienceType;

struct crateCreationEntry
{
	AsciiString crateName;
	Real crateChance;
};

typedef std::list< crateCreationEntry >											crateCreationEntryList;
typedef std::list< crateCreationEntry >::iterator						crateCreationEntryIterator;
typedef std::list< crateCreationEntry >::const_iterator			crateCreationEntryConstIterator;

/** 
		A CrateTemplate is a ini defined set of conditions plus a ThingTemplate that is the Object
		containing the correct CrateCollide module.
*/
class CrateTemplate : public Overridable
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CrateTemplate, "CrateTemplate"  )

public:
	CrateTemplate();
	// virtual destructor declared by memory pool

	void setName( AsciiString name ) { m_name = name; }
	AsciiString getName(){ return m_name; }

	inline const FieldParse *getFieldParse() const { return TheCrateTemplateFieldParseTable; }
	static const FieldParse TheCrateTemplateFieldParseTable[];		///< the parse table for INI definition

	static void parseCrateCreationEntry( INI* ini, void *instance, void *store, const void* /*userData*/ );

	AsciiString m_name;													///< name for this CrateTemplate

	Real m_creationChance;											///< Condition for random percentage chance of creating
	VeterancyLevel m_veterancyLevel;						///< Condition specifing level of killed unit
	KindOfMaskType m_killedByTypeKindof;				///< Must be killed by something with all these bits set
	ScienceType m_killerScience;								///< Must be killed by something posessing this science
	crateCreationEntryList m_possibleCrates;		///< CreationChance is for this CrateData to succeed, this list controls one-of-n crates created on success
	Bool m_isOwnedByMaker;											///< Design needs crates to be owned sometimes.

private:

};

typedef OVERRIDE<CrateTemplate> CrateTemplateOverride;


/** 
		System responsible for Crates as code objects - ini, new/delete etc
*/
class CrateSystem : public SubsystemInterface
{
public:
	CrateSystem();
	~CrateSystem();

	void init();
	void reset();
	void update(){}

	const CrateTemplate *findCrateTemplate(AsciiString name) const;
	CrateTemplate *friend_findCrateTemplate(AsciiString name);

	CrateTemplate *newCrateTemplate( AsciiString name );
	CrateTemplate *newCrateTemplateOverride( CrateTemplate *crateToOverride );

	

	static void parseCrateTemplateDefinition(INI* ini);

private:
	std::vector<CrateTemplate *> m_crateTemplateVector;

};

extern CrateSystem *TheCrateSystem;
#endif
