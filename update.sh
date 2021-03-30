
#---------------------------------------------------------------------------------------------------------------
# CONFIG

REPOS=("git@gitlab.tugraz.at:oop1-ss21/a1_public.git" "git@gitlab.tugraz.at:oop1-ss21/upstream.git" "git@gitlab.tugraz.at:oop1-ss21/implementation.git")

rootSource="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
src="${rootSource}/target/x86_64-unknown-linux-musl/release/" 

testrunner="testrunner"

path="/tmp/prog/.updateTestrunner"

#---------------------------------------------------------------------------------------------------------------


if [ "$#" -ne 1 ] ; then
  echo '[Err] Usage: ./script  "commit message" '
  exit 1
fi



cd ${rootSource}/src
cargo build --release --target x86_64-unknown-linux-musl

rm -Rf $path
mkdir -p $path

cd $path


for repo in ${REPOS[@]} ; do 
    git clone $repo
done 

cwd=$(pwd)

for dir in $(ls) ; do 
    
    cd $dir
    files=$(find . -type f -iname *${testrunner})

    for f in ${files[@]} ; do 
        rm -f $f
        cp -f $src/$testrunner $f
        git add $f
    done
    
    git status
    cd $cwd
done

    
echo "Press Enter to push files"

read -n 1 -p "Input Selection:" "mainmenuinput" key


if [[ $? -eq 0 && ${key} -eq 0 ]];        # if input == ENTER key
then

    cd $cwd

    for dir in $(ls) ; do 
        
        cd $dir
        git commit -m " ${1}"
        git push
        cd $cwd
    done

fi
