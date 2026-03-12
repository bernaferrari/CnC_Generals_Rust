#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000


/*
**	This header defines generally unused member structures used by the INI class.
**	Previously these were member structures of the INI class but they were separated
**	to help reduce header dependancies. -ehc
*/

#include	"listnode.h"
#include	"index.h"
#include "crc.h"


/*
**	The value entries for the INI file are stored as objects of this type.
**	The entry identifier and value string are combined into this object.
*/
struct INIEntry : public Node<INIEntry *> {
	INIEntry(char * entry = NULL, char * value = NULL) : Entry(entry), Value(value) {}
	~INIEntry(void);
//	~INIEntry(void) {free(Entry);Entry = NULL;free(Value);Value = NULL;}
//	int Index_ID(void) const {return(CRCEngine()(Entry, strlen(Entry)));};
	int Index_ID(void) const { return CRC::String(Entry);};

	char * Entry;
	char * Value;
};

/*
**	Each section (bracketed) is represented by an object of this type. All entries
**	subordinate to this section are attached.
*/
struct INISection : public Node<INISection *> {
		INISection(char * section) : Section(section) {}
		~INISection(void);
//		~INISection(void) {free(Section);Section = 0;EntryList.Delete();}
		INIEntry * Find_Entry(char const * entry) const;
//		int Index_ID(void) const {return(CRCEngine()(Section, strlen(Section)));};
		int Index_ID(void) const { return CRC::String(Section); }; 

		char * Section;
		List<INIEntry *> EntryList;
		IndexClass<int, INIEntry *> EntryIndex;

	private:
		INISection(INISection const & rvalue);
		INISection operator = (INISection const & rvalue);
};


