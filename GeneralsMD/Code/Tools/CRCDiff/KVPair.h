// ---------------------------------------------------------------------------
// File: KVPair.h
// Author: Matthew D. Campbell
// Creation Date: 9/4/2002
// Description: Key/Value Pair class
// ---------------------------------------------------------------------------

#ifndef __KVPAIR_H__
#define __KVPAIR_H__

#include <map>
#include <string>

typedef std::map<std::string, std::string> KeyValueMap;

class KVPairClass
{
public:
	KVPairClass( void );
	KVPairClass( const std::string& in, const std::string& delim );
	void set( const std::string& in, const std::string& delim );
	void readFromFile( const std::string& in, const std::string& delim );

	std::string getStringVal( const std::string& key ) const;

	bool getString( const std::string& key, std::string& val ) const;
	bool getInt( const std::string& key, int& val ) const;
	bool getUnsignedInt( const std::string& key, unsigned int& val ) const;

protected:
	KeyValueMap m_map;
};

#endif // __KVPAIR_H__

