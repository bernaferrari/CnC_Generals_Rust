// ---------------------------------------------------------------------------
// File: expander.h
// Author: Matthew D. Campbell
// Creation Date: 9/13/2002
// Description: Key/value pair template expansion class
// ---------------------------------------------------------------------------

#ifndef __EXPANDER_H__
#define __EXPANDER_H__

#include <map>
#include <hash_map>
#include <string>

typedef std::map<std::string, std::string> ExpansionMap;

class Expander
{
	public:
		Expander( const std::string& leftMarker, const std::string& rightMarker );

		void addExpansion( const std::string& key, const std::string val );
		void clear( void );

		void expand( const std::string& input,
				std::string& output,
				bool stripUnknown = false );

	protected:
		ExpansionMap m_expansions;
		std::string m_left;
		std::string m_right;
};

#endif // __EXPANDER_H__

