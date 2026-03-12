#pragma once

#ifndef __LANGUAGEFILTER_H
#define __LANGUAGEFILTER_H

#include "Common/STLTypedefs.h"
#include "Common/AsciiString.h"
#include "Common/UnicodeString.h"

class File;

struct AsciiStringLessThan
{
	Bool operator()(AsciiString a, AsciiString b) const
	{
		return (a.compareNoCase(b) < 0);
	}
};

struct UnicodeStringLessThan
{
	Bool operator()(UnicodeString a, UnicodeString b) const
	{
		return (a.compareNoCase(b) < 0);
	}
};

struct UnicodeStringsEqual
{
	Bool operator()(UnicodeString a, UnicodeString b) const
	{
		Bool retval = (a.compareNoCase(b) == 0);
		DEBUG_LOG(("Comparing %ls with %ls, return value is ", a.str(), b.str()));
		if (retval) {
			DEBUG_LOG(("true.\n"));
		} else {
			DEBUG_LOG(("false.\n"));
		}
		return retval;
	}
};

typedef std::map<UnicodeString, Bool, UnicodeStringLessThan> LangMap;
typedef std::map<UnicodeString, Bool, UnicodeStringLessThan>::iterator LangMapIter;

static const int LANGUAGE_XOR_KEY = 0x5555;
static const char BadWordFileName[] = "langdata.dat";

class LanguageFilter : public SubsystemInterface {
public:
	LanguageFilter();
	~LanguageFilter();

	void init();
	void reset();
	void update();
	void filterLine(UnicodeString &line);

protected:
	Bool readWord(File *file1, UnsignedShort *buf);
	void unHaxor(UnicodeString &word);
	LangMap m_wordList;
	LangMap m_subWordList;
};

extern LanguageFilter *TheLanguageFilter;
LanguageFilter * createLanguageFilter();

#endif //#define __LANGUAGEFILTER_H
